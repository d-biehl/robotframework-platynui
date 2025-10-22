from robot.api.deco import library
from robotlibcore import DynamicCore, keyword

from ..__version__ import __version__


@library(
    scope='GLOBAL',
    version=__version__,
    converters={},
)
class BareMetal(DynamicCore):
    """PlatynUI.BareMetal is a library for Robot Framework to automate and test graphical user interfaces (GUIs) using the PlatynUI native backend."""

    def __init__(self) -> None:
        pass

    @keyword
    def query(
        self,
        expression: str,
    ) -> None:
        """Evaluates a UI query against the current desktop.

        This is a placeholder method and is not implemented yet.
        """
