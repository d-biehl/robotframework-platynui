// Same self-bootstrapping story as the Java fixtures under apps/: the wrapper
// CLIENT runs on the PATH `java` (8+), the Gradle DAEMON gets a JVM 17+ from the
// committed daemon JVM criteria (gradle/gradle-daemon-jvm.properties: Temurin 21,
// auto-provisioned via the embedded Foojay URLs — regenerate with
// `gradlew updateDaemonJvm --jvm-version=21`). The build JVM is independent of
// what the agent targets (Java 8 bytecode) — that comes from the toolchain in
// build.gradle.kts.
plugins {
    // Auto-provisions the pinned JDK 21 compile toolchain.
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

rootProject.name = "platynui-agent"
