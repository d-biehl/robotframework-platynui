import warnings

from robot.api.deco import library

from .__version__ import __version__
from ._our_libcore import OurDynamicCore, keyword


@library(
    scope='GLOBAL',
    version=__version__,
    converters={},
)
class PlatynUI(OurDynamicCore):
    """PlatynUI is a library for Robot Framework to automate and test graphical user interfaces (GUIs) using the PlatynUI native backend.

    It provides keywords to interact with UI elements, perform actions,
    and verify the state of the application under test.
    """

    def __init__(self) -> None:
        super().__init__([])

        warnings.warn('The PlatynUI library is not implemented yet. This is a placeholder.')

    @keyword
    def dummy_keyword(self) -> None:
        """A no-op example keyword.

        This placeholder keyword demonstrates how library keywords are exposed and documented.

        Examples:
        | Dummy Keyword |

        Tags: example, internal
        """
        pass
