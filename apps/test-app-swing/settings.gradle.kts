// The wrapper CLIENT runs on the PATH `java` (8+); the Gradle DAEMON needs a
// JVM 17+ and gets it from the committed daemon JVM criteria
// (gradle/gradle-daemon-jvm.properties: Temurin 21, auto-provisioned via the
// embedded Foojay URLs — regenerate with `gradlew updateDaemonJvm
// --jvm-version=21`). The build JVM is independent of the fixture's target
// (Java 8 bytecode) and launch runtime (provisioned Temurin 8) — those come
// from the toolchains below.
plugins {
    // Auto-provisions the pinned toolchains (JDK 21 compile, Java 8 launch runtime).
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

rootProject.name = "test-app-swing"
