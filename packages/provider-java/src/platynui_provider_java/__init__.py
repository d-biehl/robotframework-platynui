"""Delivery package for PlatynUI's in-JVM Java agent.

This package carries one artifact — ``platynui-agent.jar``, the agent PlatynUI loads into a target
JVM to read Swing/JavaFX/SWT models from the inside. It contains no automation logic of its own.

**Installing it is the consent for in-JVM instrumentation.** Without it, PlatynUI reports Java agent
support as unavailable and instruments nothing; uninstalling removes the capability entirely. That
is why it is a separate package rather than a flag: an environment either has the artifact or it
does not, and that decision is visible in the environment's own dependency list.

The JAR is platform- and Python-version-neutral, so this is a ``py3-none-any`` wheel — it is not
duplicated into the per-platform native wheels, and building those needs no JDK.
"""

from pathlib import Path

from .__version__ import __version__

__all__ = ['__version__', 'agent_jar', 'provider_info']

#: Where the JAR sits inside the installed package.
_AGENT_JAR = Path(__file__).parent / 'agent' / 'platynui-agent.jar'


def agent_jar() -> Path:
    """Return the path of the bundled agent JAR.

    Raises:
        FileNotFoundError: if the package was assembled without its artifact. That is a broken
            install rather than a missing feature, so it is reported loudly instead of being
            reported as "no Java support".
    """
    if not _AGENT_JAR.is_file():
        msg = (
            f'the PlatynUI agent JAR is missing from this installation (expected at {_AGENT_JAR}) — '
            'reinstall platynui-provider-java'
        )
        raise FileNotFoundError(msg)
    return _AGENT_JAR


def provider_info() -> dict[str, str]:
    """Return what a PlatynUI runtime needs to use this package.

    This is the target of the ``platynui.providers`` entry point, and the one shape both discovery
    transports read: the in-process lookup calls it directly, the standalone binaries call it
    through a one-shot query in the environment's own interpreter.

    The version travels with the path on purpose. Provider and agent must match **exactly** — an
    agent cannot be unloaded from a JVM, so a mismatch has exactly one remedy (restart the
    application) and is worth catching before a connection rather than mid-test.
    """
    return {'agent_jar': str(agent_jar()), 'version': __version__}
