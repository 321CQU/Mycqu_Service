use std::{
    future::Future,
    hash::Hash,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use dashmap::DashMap;
use rand::prelude::IndexedRandom;
use rsmycqu::session::Client;
use serde::Deserialize;
use tokio::{
    sync::{RwLock, oneshot},
    time::interval,
};
use tonic::Status;
// --- Proxied Client Provider ---

#[derive(Deserialize, Debug)]
struct ProxyApiResponse {
    data: ProxyApiData,
}

#[derive(Deserialize, Debug)]
struct ProxyApiData {
    proxy_list: Vec<String>,
}

struct ClientCache {
    clients: Vec<Arc<Client>>,
    expires_at: Instant,
}

impl Default for ClientCache {
    fn default() -> Self {
        Self {
            clients: Vec::new(),
            expires_at: Instant::now(),
        }
    }
}

pub struct ProxiedClientProvider {
    cache: RwLock<ClientCache>,
    coalescer: RequestCoalescer<String, Result<(), String>>,
    http_client: reqwest::Client, // For fetching proxies from the API
}

impl ProxiedClientProvider {
    fn new() -> Self {
        Self {
            cache: RwLock::default(),
            coalescer: RequestCoalescer::new(),
            http_client: reqwest::Client::builder().no_proxy().build().unwrap(),
        }
    }

    async fn refresh_clients(&self) -> Result<(), String> {
        // 1. Fetch proxy URLs from API
        let secret_id =
            std::env::var("PROXY_SECRET_ID").map_err(|_| "PROXY_SECRET_ID not set".to_string())?;
        let signature =
            std::env::var("PROXY_SIGNATURE").map_err(|_| "PROXY_SIGNATURE not set".to_string())?;

        const PROXY_API_URL_TEMPLATE: &str = "https://dps.kdlapi.com/api/getdps/?secret_id={secret_id}&signature={signature}&num=1&format=json&sep=1&f_et=1&area=%E9%87%8D%E5%BA%86";
        let url = PROXY_API_URL_TEMPLATE
            .replace("{secret_id}", &secret_id)
            .replace("{signature}", &signature);

        let resp = self
            .http_client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let api_data: ProxyApiResponse = resp.json().await.map_err(|e| e.to_string())?;

        if api_data.data.proxy_list.is_empty() {
            return Err("No proxies fetched from API".to_string());
        }

        // 2. Parse proxies and create clients
        let mut new_clients = Vec::new();
        let mut min_expire = u64::MAX;
        let username =
            std::env::var("PROXY_USERNAME").map_err(|_| "PROXY_USERNAME not set".to_string())?;
        let password =
            std::env::var("PROXY_PASSWORD").map_err(|_| "PROXY_PASSWORD not set".to_string())?;

        for item in &api_data.data.proxy_list {
            let parts: Vec<&str> = item.split(',').collect();
            if parts.len() == 2 {
                let ip_port = parts[0].trim();
                if let Ok(expire) = parts[1].trim().parse::<u64>() {
                    if expire < min_expire {
                        min_expire = expire;
                    }
                    let proxy_url = format!("http://{}", ip_port);
                    let proxy = reqwest::Proxy::all(&proxy_url)
                        .map_err(|e| format!("Invalid proxy URL '{}': {}", proxy_url, e))?
                        .basic_auth(&username, &password);

                    let client = Client::custom(|builder| {
                        builder
                            .pool_max_idle_per_host(10)
                            .pool_idle_timeout(Duration::from_secs(30))
                            .tcp_keepalive(Duration::from_secs(30))
                            .proxy(proxy)
                    })
                    .map_err(|e| format!("Failed to create proxied client: {}", e))?;

                    new_clients.push(Arc::new(client));
                }
            }
        }

        if new_clients.is_empty() {
            return Err("Failed to parse any valid proxies from API response".to_string());
        }

        // 3. Update cache
        let ttl = Duration::from_secs(min_expire.saturating_sub(10).max(10));
        let mut cache_writer = self.cache.write().await;
        cache_writer.clients = new_clients;
        cache_writer.expires_at = Instant::now() + ttl;

        Ok(())
    }

    pub async fn get_random_client(&self) -> Option<Arc<Client>> {
        // 快速路径：先检查缓存是否有效，避免不必要的 coalescer 开销
        let maybe_needs_refresh = {
            let cache = self.cache.read().await;
            Instant::now() >= cache.expires_at || cache.clients.is_empty()
        };

        // 只有在可能需要刷新时才进入 coalescer
        if maybe_needs_refresh {
            let refresh_result = self
                .coalescer
                .execute("refresh_clients".to_string(), || async {
                    // 在 coalescer 内部再次检查是否需要刷新（双重检查）
                    // 这样可以避免多个请求同时判断需要刷新后都触发 API 调用
                    let needs_refresh = {
                        let cache = self.cache.read().await;
                        Instant::now() >= cache.expires_at || cache.clients.is_empty()
                    };

                    if needs_refresh {
                        self.refresh_clients().await
                    } else {
                        Ok(())
                    }
                })
                .await;

            if let Err(e) = refresh_result {
                eprintln!("Failed to refresh proxied clients: {}", e);
            }
        }

        let cache = self.cache.read().await;
        cache.clients.choose(&mut rand::rng()).cloned()
    }
}

pub static PROXIED_CLIENT_PROVIDER: LazyLock<ProxiedClientProvider> =
    LazyLock::new(ProxiedClientProvider::new);

pub static PROXY_CLIENT_GET_ERROR: LazyLock<Status> =
    LazyLock::new(|| Status::internal("Failed to get proxied client"));

// --- Generic Caching & Coalescing Utilities ---

pub struct ExpiringDashMap<K, V> {
    inner: Arc<DashMap<K, (V, Instant)>>,
    /// 后台清扫句柄，drop 时自动停止
    _handle: tokio::task::JoinHandle<()>,
}

impl<K, V> ExpiringDashMap<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 新建 map，每 `check_interval` 检查一次过期键
    pub fn new(check_interval: Duration) -> Arc<Self> {
        let map = Arc::new(DashMap::new());
        let map_clone = map.clone();
        let handle = tokio::spawn(async move {
            let mut ticker = interval(check_interval);
            loop {
                ticker.tick().await;
                // 简单全表扫描；量大可优化为分片或分层
                map_clone.retain(|_, (_, exp)| *exp > Instant::now());
            }
        });
        Arc::new(ExpiringDashMap {
            inner: map,
            _handle: handle,
        })
    }

    /// 插入，指定 TTL
    pub fn insert(&self, key: K, value: V, ttl: Duration) {
        let expire = Instant::now() + ttl;
        self.inner.insert(key, (value, expire));
    }

    /// 读取，自动过滤过期
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner
            .get(key)
            .filter(|entry| entry.value().1 > Instant::now())
            .map(|entry| entry.value().0.clone())
    }

    /// 删除
    #[allow(dead_code)]
    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner.remove(key).map(|(_, (v, _))| v)
    }

    /// 手动立即清理过期键（可选）
    #[allow(dead_code)]
    pub fn purge_expired(&self) {
        self.inner.retain(|_, (_, exp)| *exp > Instant::now());
    }
}

/// 通用的请求合并器
pub struct RequestCoalescer<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    ongoing_requests: DashMap<K, Vec<oneshot::Sender<V>>>,
}

impl<K, V> RequestCoalescer<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            ongoing_requests: DashMap::new(),
        }
    }

    /// 执行一个可能被合并的异步任务
    pub async fn execute<F, Fut>(&self, key: K, task: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V> + Send,
    {
        let (tx, rx) = oneshot::channel();

        let mut entry = self.ongoing_requests.entry(key.clone()).or_default();
        entry.push(tx);

        if entry.len() == 1 {
            drop(entry);
            let result = task().await;
            if let Some(waiters) = self.ongoing_requests.remove(&key) {
                for waiter_tx in waiters.1 {
                    let _ = waiter_tx.send(result.clone());
                }
            }
            return result;
        }

        drop(entry);
        rx.await.expect("Leader should always send a response")
    }
}

/// 集成了缓存的请求合并器
pub struct CachedCoalescer<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    cache: Arc<ExpiringDashMap<K, V>>,
    coalescer: RequestCoalescer<K, V>,
}

impl<K, V> CachedCoalescer<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(check_interval: Duration) -> Self {
        Self {
            cache: ExpiringDashMap::new(check_interval),
            coalescer: RequestCoalescer::new(),
        }
    }

    /// 执行一个可能被缓存或合并的异步任务
    pub async fn execute<F, Fut>(&self, key: K, task: F, ttl: Duration) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V> + Send,
    {
        if let Some(value) = self.cache.get(&key) {
            return value;
        }

        let key_clone = key.clone();
        let task_fut = task();

        let coalescer_task = || async move {
            let result = task_fut.await;
            self.cache.insert(key_clone, result.clone(), ttl);
            result
        };

        self.coalescer.execute(key, coalescer_task).await
    }

    pub fn clean_cache(&self, key: &K) {
        self.cache.remove(key);
    }
}
