// The PlatynUI Java agent — the artifact that gets loaded INTO a target JVM
// (OpenSpec change `java-agent-core`). Toolchain policy mirrors the Java
// fixtures under apps/: build on an auto-provisioned JDK 21, emit Java 8
// bytecode. Targeting 8 is not nostalgia — enterprise Swing applications still
// run on 8, and an agent that cannot load there is useless for them.
//
// The agent depends on nothing beyond the JDK APIs: it is loaded into a foreign
// process, so every extra jar on its classpath would be a jar the target
// application did not ask for. JSON framing and the RPC server are therefore
// hand-rolled against `java.base` only. That rule covers the PRODUCT — the test
// sources below use JUnit, which never reaches the JAR.
plugins {
    java
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(21)
    }
}

repositories {
    // Test dependencies only; nothing from here ends up in the agent JAR.
    mavenCentral()
}

dependencies {
    testImplementation(platform("org.junit:junit-bom:5.11.4"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

tasks.compileJava {
    options.release = 8
    options.encoding = "UTF-8"
    // Targeting 8 on a JDK 21 compiler — mute the "source/target 8 is obsolete" nag,
    // keep every other warning fatal: this code runs inside somebody else's process.
    options.compilerArgs.addAll(listOf("-Xlint:all,-options", "-Werror"))
}

// The tests exercise the product's Java 8 sources but need not themselves be
// Java 8: they run on the build toolchain, where JUnit 5 lives.
tasks.compileTestJava {
    options.release = 17
    options.encoding = "UTF-8"
}

tasks.test {
    useJUnitPlatform()
    // The Swing tests build real components to read their model and layout, never
    // to show them. Forcing headless keeps that honest — and keeps the suite
    // runnable on a build machine with no desktop.
    systemProperty("java.awt.headless", "true")
    testLogging {
        events("failed")
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
}

// The version travels three ways, all from the single `version` in
// gradle.properties: the manifest (for humans and tooling), a generated resource
// (what the agent reports in the handshake file — read via getResourceAsStream,
// which works no matter which class loader ends up defining the agent package),
// and the JAR file name, which deliberately stays version-LESS so consumers can
// address it by a stable path.
val generateVersionResource = tasks.register("generateVersionResource") {
    description = "Write the agent version into a resource the agent reads at runtime"
    val outputDir = layout.buildDirectory.dir("generated/resources/version")
    val agentVersion = providers.provider { project.version.toString() }
    inputs.property("version", agentVersion)
    outputs.dir(outputDir)
    doLast {
        val file = outputDir.get().file("platynui/agent/version.properties").asFile
        file.parentFile.mkdirs()
        file.writeText("version=${agentVersion.get()}\n")
    }
}

sourceSets {
    main {
        resources.srcDir(generateVersionResource)
    }
}

tasks.jar {
    // Stable, version-less name: the wheel, the `-javaagent:` command line and the
    // discovery path all address this file by name (design 9).
    archiveFileName = "platynui-agent.jar"
    manifest {
        attributes(
            // Both entry points, one artifact (design 1): `premain` for `-javaagent`
            // at launch, `agentmain` for attach into a running JVM.
            "Premain-Class" to "platynui.agent.Agent",
            "Agent-Class" to "platynui.agent.Agent",
            // The agent reads accessibility/scene models; it never rewrites app
            // logic, so it asks for no instrumentation capabilities.
            "Can-Redefine-Classes" to "false",
            "Can-Retransform-Classes" to "false",
            // Convenience attach driver for hosts that happen to have a JDK
            // (`java -jar platynui-agent.jar <pid>`); the normal path is the
            // native Rust attach transport, which needs no JDK at all (design 5).
            "Main-Class" to "platynui.agent.AttachDriver",
            "Implementation-Title" to "PlatynUI Java Agent",
            "Implementation-Version" to project.version.toString(),
        )
    }
}

// `just build-java-agent` calls this; `assemble` would also drag in distributions
// we do not produce.
tasks.register("agentJar") {
    description = "Build the PlatynUI agent JAR (build/libs/platynui-agent.jar)"
    group = "build"
    dependsOn(tasks.jar)
}
