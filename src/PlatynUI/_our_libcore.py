import inspect
from typing import Any, Optional, cast

from assertionengine import AssertionOperator, verify_assertion
from robotlibcore import DynamicCore, keyword

from ._assertable import PLATYNUI_ASSERTABLE_FIELD

__all__ = ['OurDynamicCore', 'keyword']


def _get_args_index(args_list: list[str | tuple[str] | tuple[str, Any]]) -> int:
    return next(
        (i for i, r in enumerate(args_list) if (isinstance(r, str) and r.startswith('*')) or r[0].startswith('**')),
        len(args_list),
    )


_assertion_parameters = [
    inspect.Parameter(
        'assertion_operator',
        inspect.Parameter.POSITIONAL_OR_KEYWORD,
        annotation=AssertionOperator | None,
        default=None,
    ),
    inspect.Parameter(
        'assertion_expected',
        inspect.Parameter.POSITIONAL_OR_KEYWORD,
        annotation=Any,
        default=None,
    ),
    inspect.Parameter(
        'assertion_message',
        inspect.Parameter.POSITIONAL_OR_KEYWORD,
        annotation=str | None,
        default=None,
    ),
]


def _add_assertion_parameters(sig: inspect.Signature) -> inspect.Signature:
    # Originale Parameter in Kategorien aufteilen
    positional_only = []
    positional_or_keyword = []
    var_positional = None
    keyword_only = []
    var_keyword = None

    for param in sig.parameters.values():
        if param.kind == inspect.Parameter.POSITIONAL_ONLY:
            positional_only.append(param)
        elif param.kind == inspect.Parameter.POSITIONAL_OR_KEYWORD:
            positional_or_keyword.append(param)
        elif param.kind == inspect.Parameter.VAR_POSITIONAL:
            var_positional = param
        elif param.kind == inspect.Parameter.KEYWORD_ONLY:
            keyword_only.append(param)
        elif param.kind == inspect.Parameter.VAR_KEYWORD:
            var_keyword = param

    # Neue Reihenfolge der Parameter erstellen
    updated_parameters = (
        positional_only
        + positional_or_keyword
        + _assertion_parameters  # Neue Parameter hinzufügen
        + ([var_positional] if var_positional else [])
        + keyword_only
        + ([var_keyword] if var_keyword else [])
    )

    # Neue Signatur erstellen
    return inspect.Signature(parameters=updated_parameters)  # pyright: ignore[reportUnknownArgumentType]


class OurDynamicCore(DynamicCore):
    def get_keyword_arguments(self, name: str) -> Any:
        result: list[str | tuple[str] | tuple[str, Any]] = super().get_keyword_arguments(name)

        kw = self.keywords.get(name)

        if kw is not None and getattr(cast('object', kw), PLATYNUI_ASSERTABLE_FIELD, False):
            kwargs_index = _get_args_index(result)  # pyright: ignore[reportUnknownArgumentType]
            result.insert(kwargs_index, ('assertion_operator', None))
            result.insert(kwargs_index + 1, ('assertion_expected', None))
            result.insert(kwargs_index + 2, ('assertion_message', None))
        return result

    def get_keyword_types(self, name: str) -> Any:
        result = super().get_keyword_types(name)

        kw = self.keywords.get(name)

        if kw is not None and getattr(cast('object', kw), PLATYNUI_ASSERTABLE_FIELD, False):
            result['assertion_operator'] = Optional[AssertionOperator]  # noqa: UP045
            result['assertion_expected'] = Any
            result['assertion_message'] = Optional[str]  # noqa: UP045

        return result

    def run_keyword(self, name: str, args: Any, kwargs: Any = None) -> Any:
        kw = self.keywords.get(name)
        if kw is not None and getattr(cast('object', kw), PLATYNUI_ASSERTABLE_FIELD, False):

            def do_verify(
                *args: Any,
                assertion_operator: AssertionOperator | None = None,
                assertion_expected: Any | None = None,
                assertion_message: str | None = None,
                **kwargs: Any,
            ) -> Any:
                return verify_assertion(
                    self.keywords[name](*args, **(kwargs or {})),  # pyright: ignore[reportUnknownArgumentType]
                    assertion_operator,
                    assertion_expected,
                    name,
                    assertion_message,
                )

            new_sig = _add_assertion_parameters(inspect.signature(kw))  # pyright: ignore[reportUnknownArgumentType]

            bound_args = new_sig.bind(*args, **(kwargs or {}))
            bound_args.apply_defaults()

            assertion_operator = bound_args.arguments.pop('assertion_operator', None)
            assertion_expected = bound_args.arguments.pop('assertion_expected', None)
            assertion_message = bound_args.arguments.pop('assertion_message', None)

            return do_verify(
                *bound_args.args,
                assertion_operator=assertion_operator,
                assertion_expected=assertion_expected,
                assertion_message=assertion_message,
                **(bound_args.kwargs or {}),
            )

        return self.keywords[name](*args, **(kwargs or {}))
