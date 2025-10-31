from typing import Any, cast

from robotlibcore import DynamicCore, keyword

__all__ = ['OurDynamicCore', 'keyword']


class OurDynamicCore(DynamicCore):
    """Extended DynamicCore that serves as base for Robot Framework libraries."""

    def get_keyword_source(self, keyword_name: str) -> str | None:
        """Return keyword source information prioritising decorator metadata."""
        raw_method = self.keywords.get(keyword_name)
        if raw_method is not None:
            method = cast(Any, raw_method)
            source = getattr(method, 'robot_source', None)
            lineno = getattr(method, 'robot_lineno', None)

            if isinstance(source, str) and isinstance(lineno, int):
                return f'{source}:{lineno}'
            if isinstance(source, str):
                return source
            if isinstance(lineno, int):
                return f':{lineno}'

        fallback = super().get_keyword_source(keyword_name)
        if fallback is None or isinstance(fallback, str):
            return fallback
        return str(fallback)
