# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

# pyright: reportUnusedImport=false

"""Default proxy registry.

Importing this package side-effect-registers every default proxy with
`PatternProxyFactory` via the `@pattern_proxy_for` decorator. This is
the **one** locally permitted import-side-effect in the codebase.
"""

from . import (  # noqa: F401  -- side-effect imports
    base,
    buttons,
    combobox,
    item,
    text,
)
