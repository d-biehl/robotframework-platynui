// Toolchain policy (OpenSpec `migrate-swing-test-app-to-gradle`): build on an
// auto-provisioned JDK 21, target Java 8 bytecode, run on a genuine Java 8
// runtime by default. The fixture depends on nothing beyond the JDK APIs, and
// the sources keep their pre-Gradle layout (`src/`, not `src/main/java`).
plugins {
    java
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(21)
    }
}

sourceSets {
    main {
        java.setSrcDirs(listOf("src"))
    }
}

tasks.compileJava {
    options.release = 8
    options.encoding = "UTF-8"
    // Targeting 8 is the point of this fixture — mute the JDK 21 compiler's
    // "source/target 8 is obsolete" nag.
    options.compilerArgs.add("-Xlint:-options")
}

val java8Launcher = javaToolchains.launcherFor {
    languageVersion = JavaLanguageVersion.of(8)
}
val java21Launcher = javaToolchains.launcherFor {
    languageVersion = JavaLanguageVersion.of(21)
}

// Consumers (`just` recipes, acceptance lane) launch the compiled classes with
// their own `java -D… -cp` command line; this file tells them where the
// provisioned JVMs live: `java8` is the default launch runtime, `java21` the
// compile toolchain for the dual-runtime smoke. Forward slashes so the file
// parses as `.properties` (backslash is its escape character).
val writeJavaLaunchers = tasks.register("writeJavaLaunchers") {
    description = "Write the provisioned Java launcher paths to build/java-launchers.properties"
    val outputFile = layout.buildDirectory.file("java-launchers.properties")
    // Resolved into local providers so the doLast lambda captures no script
    // object references (configuration-cache requirement).
    val java8Path = java8Launcher.map { it.executablePath.toString().replace('\\', '/') }
    val java21Path = java21Launcher.map { it.executablePath.toString().replace('\\', '/') }
    inputs.property("java8", java8Path)
    inputs.property("java21", java21Path)
    outputs.file(outputFile)
    doLast {
        outputFile.get().asFile.writeText("java8=${java8Path.get()}\njava21=${java21Path.get()}\n")
    }
}

tasks.build {
    dependsOn(writeJavaLaunchers)
}

// Ad-hoc launch on the default (Java 8) runtime: `gradlew run --args="--auto-close 3"`.
// On Windows the Access Bridge is enabled for this process only (same flag the
// tests pass). Test consumers do NOT use this task — they spawn `java` directly
// so they control the bridge flag per launch and own the process PID.
tasks.register<JavaExec>("run") {
    description = "Launch the Swing test app on the provisioned Java 8 runtime"
    group = "application"
    classpath = sourceSets.main.get().runtimeClasspath
    mainClass = "platynui.testapp.Main"
    javaLauncher = java8Launcher
    if (System.getProperty("os.name").lowercase().contains("windows")) {
        systemProperty("javax.accessibility.assistive_technologies", "com.sun.java.accessibility.AccessBridge")
    }
}
