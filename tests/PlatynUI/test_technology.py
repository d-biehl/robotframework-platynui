# SPDX-FileCopyrightText: 2024 Daniel Biehl <daniel.biehl@imbus.de>
#
# SPDX-License-Identifier: Apache-2.0

"""Tests for ``PlatynUI.core.technology``."""

from PlatynUI.core import Technology


def test_subclass_default_name_is_qualname() -> None:
    class RustTechnology(Technology):
        pass

    assert RustTechnology().name == 'test_subclass_default_name_is_qualname.<locals>.RustTechnology'


def test_subclass_can_override_name() -> None:
    class JsonRpc(Technology):
        @property
        def name(self) -> str:
            return 'json-rpc'

    assert JsonRpc().name == 'json-rpc'
