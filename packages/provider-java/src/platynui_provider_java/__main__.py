"""``platynui-provider-java`` — tell the operator where the agent JAR is.

PlatynUI finds the JAR by itself; this command exists for the case PlatynUI cannot help with: a
target JVM where dynamic attach is blocked, so the agent has to be put on the launch command line
by hand.

    java -javaagent:$(platynui-provider-java agent-path) -jar theirapp.jar
"""

import argparse
import json
import sys

from . import agent_jar, provider_info


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog='platynui-provider-java', description=__doc__.splitlines()[0])
    subcommands = parser.add_subparsers(dest='command', required=True)
    subcommands.add_parser('agent-path', help='print the path of the bundled agent JAR')
    subcommands.add_parser('info', help='print the provider manifest as JSON')

    arguments = parser.parse_args(argv)
    try:
        if arguments.command == 'agent-path':
            print(agent_jar())
        else:
            print(json.dumps(provider_info()))
    except FileNotFoundError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
