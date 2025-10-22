import warnings

from robot.api.deco import library
from robotlibcore import DynamicCore, keyword

from .__version__ import __version__


@library(
    scope='GLOBAL',
    version=__version__,
    converters={},
)
class PlatynUI(DynamicCore):
    """PlatynUI is a library for Robot Framework to automate and test graphical user interfaces (GUIs) using the PlatynUI native backend.

    It provides keywords to interact with UI elements, perform actions,
    and verify the state of the application under test.
    """

    def __init__(self) -> None:
        super().__init__([])

        warnings.warn('The PlatynUI library is not implemented yet. This is a placeholder.')

    @keyword
    def dummy_keyword(self) -> None:
        """A dummy keyword to illustrate the structure of the library."""
        pass
