import os

from _321CQU.tools import ConfigHandler

__all__ = ['ConfigReader', 'BASE_DIR']

BASE_DIR = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))


class ConfigReader(ConfigHandler):
    def __init__(self):
        super().__init__(str(BASE_DIR) + '/utils/config/basic_config.cfg')
