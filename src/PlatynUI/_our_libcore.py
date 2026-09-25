from typing import Any, cast

from robotlibcore import DynamicCore, keyword

from .core.native_logging import flush_native_logs

__all__ = ['OurDynamicCore', 'keyword']


class OurDynamicCore(DynamicCore):
    """Extended DynamicCore that serves as base for Robot Framework libraries."""

    def run_keyword(self, name: str, args: list[Any], kwargs: dict[str, Any] | None = None) -> Any:
        """Run a keyword with the native log records around it delivered into its log.

        Robot Framework records messages only from the thread that runs the keyword, so
        native records are delivered here: those queued since the last keyword (by native
        background threads) before it runs, and the keyword's own afterwards.
        """
        flush_native_logs()
        try:
            return super().run_keyword(name, args, kwargs)
        finally:
            flush_native_logs()

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

        fallback: str | None = super().get_keyword_source(keyword_name)
        return fallback
