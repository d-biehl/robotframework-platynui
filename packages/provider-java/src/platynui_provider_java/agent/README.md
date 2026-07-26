# The agent JAR is a build artifact: `just build-provider-java` stages it here
# from java/agent/build/libs/. It is deliberately not checked in — the JAR must
# always match the sources it was built from, and a stale committed copy would
# be the one thing the exact-version handshake cannot catch.
