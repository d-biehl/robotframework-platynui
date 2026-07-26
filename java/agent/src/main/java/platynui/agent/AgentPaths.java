package platynui.agent;

import java.io.IOException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.attribute.AclEntry;
import java.nio.file.attribute.AclEntryFlag;
import java.nio.file.attribute.AclEntryPermission;
import java.nio.file.attribute.AclEntryType;
import java.nio.file.attribute.AclFileAttributeView;
import java.nio.file.attribute.PosixFileAttributeView;
import java.nio.file.attribute.PosixFilePermission;
import java.nio.file.attribute.PosixFilePermissions;
import java.nio.file.attribute.UserPrincipal;
import java.util.Collections;
import java.util.EnumSet;
import java.util.Set;

/**
 * Where the handshake files live, and how the directory holding them is locked down.
 *
 * <p>This rule is a <strong>contract with the Rust side</strong>: the provider computes the same
 * path to discover agents. Any change here has to happen in {@code crates/java-agent} in the same
 * commit.
 *
 * <table>
 *   <caption>Handshake directory by platform</caption>
 *   <tr><th>Platform</th><th>Directory</th></tr>
 *   <tr><td>any</td><td>{@code $PLATYNUI_AGENT_DIR} when set — the test and dev override</td></tr>
 *   <tr><td>Windows</td><td>{@code %LOCALAPPDATA%\PlatynUI\agents}</td></tr>
 *   <tr><td>Unix, {@code XDG_RUNTIME_DIR} set</td><td>{@code $XDG_RUNTIME_DIR/platynui/agents}</td></tr>
 *   <tr><td>Unix, otherwise</td><td>{@code $TMPDIR/platynui-<user>/agents} ({@code /tmp} if unset)</td></tr>
 * </table>
 *
 * <p>{@code $TMPDIR} rather than {@code java.io.tmpdir}, because the Rust side reads the
 * environment variable and the two must agree even when the target JVM was launched with an
 * overridden {@code java.io.tmpdir}.
 *
 * <p>The directory is owner-only: POSIX {@code 0700}, and on Windows an ACL granting only the
 * owning user. That is not decoration — the file carries the connection token, and loopback TCP is
 * connectable by <em>any</em> local user (terminal servers are common in enterprise Java).
 */
final class AgentPaths {

    /** Test and dev override; also how a fixture keeps its agents out of the real directory. */
    static final String DIR_ENV = "PLATYNUI_AGENT_DIR";

    private static final Set<PosixFilePermission> OWNER_ONLY_DIR =
            EnumSet.of(PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE, PosixFilePermission.OWNER_EXECUTE);

    private static final Set<PosixFilePermission> OWNER_ONLY_FILE =
            EnumSet.of(PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE);

    private AgentPaths() {
        // Static helper.
    }

    /** The handshake directory for this user, not yet created. */
    static Path handshakeDirectory() {
        String override = System.getenv(DIR_ENV);
        if (override != null && !override.isEmpty()) {
            return Paths.get(override);
        }
        if (isWindows()) {
            String localAppData = System.getenv("LOCALAPPDATA");
            Path base = localAppData != null && !localAppData.isEmpty()
                    ? Paths.get(localAppData)
                    : Paths.get(System.getProperty("user.home"), "AppData", "Local");
            return base.resolve("PlatynUI").resolve("agents");
        }
        String runtimeDir = System.getenv("XDG_RUNTIME_DIR");
        if (runtimeDir != null && !runtimeDir.isEmpty()) {
            return Paths.get(runtimeDir).resolve("platynui").resolve("agents");
        }
        String tmpDir = System.getenv("TMPDIR");
        Path base = tmpDir != null && !tmpDir.isEmpty() ? Paths.get(tmpDir) : Paths.get("/tmp");
        return base.resolve("platynui-" + System.getProperty("user.name", "unknown")).resolve("agents");
    }

    /** The handshake file for a process id, inside {@link #handshakeDirectory()}. */
    static Path handshakeFile(Path directory, long pid) {
        return directory.resolve("agent-" + pid);
    }

    /**
     * Creates the handshake directory if needed and makes sure it is owner-only.
     *
     * <p>Hardening runs on every start, not only on creation: on Unix the fallback base is
     * {@code /tmp}, which is world-writable, so a directory that already exists is not necessarily
     * a directory <em>we</em> created. A directory we cannot lock down is an error, not a warning —
     * publishing a token into a readable directory would be worse than having no agent at all.
     *
     * @throws IOException if the directory cannot be created or cannot be made owner-only
     */
    static Path createHandshakeDirectory() throws IOException {
        Path directory = handshakeDirectory();
        createOwnerOnlyDirectories(directory);
        return directory;
    }

    private static void createOwnerOnlyDirectories(Path directory) throws IOException {
        Path parent = directory.getParent();
        if (parent != null && !Files.isDirectory(parent)) {
            createOwnerOnlyDirectories(parent);
        }
        try {
            if (supportsPosix(directory)) {
                Files.createDirectory(directory, PosixFilePermissions.asFileAttribute(OWNER_ONLY_DIR));
            } else {
                Files.createDirectory(directory);
            }
        } catch (FileAlreadyExistsException e) {
            if (!Files.isDirectory(directory)) {
                throw e;
            }
        }
        hardenDirectory(directory);
    }

    private static void hardenDirectory(Path directory) throws IOException {
        if (supportsPosix(directory)) {
            Files.setPosixFilePermissions(directory, OWNER_ONLY_DIR);
            return;
        }
        AclFileAttributeView acl = Files.getFileAttributeView(directory, AclFileAttributeView.class);
        if (acl == null) {
            // Neither POSIX nor ACLs: nothing we can enforce, and we will not publish a
            // token on a filesystem that cannot keep it.
            throw new IOException("cannot restrict access to " + directory + " on this filesystem");
        }
        UserPrincipal owner = Files.getOwner(directory);
        AclEntry entry = AclEntry.newBuilder()
                .setType(AclEntryType.ALLOW)
                .setPrincipal(owner)
                .setPermissions(EnumSet.allOf(AclEntryPermission.class))
                .setFlags(AclEntryFlag.DIRECTORY_INHERIT, AclEntryFlag.FILE_INHERIT)
                .build();
        acl.setAcl(Collections.singletonList(entry));
    }

    /** Restricts an already-created handshake file to its owner (POSIX only; Windows inherits). */
    static void hardenFile(Path file) throws IOException {
        if (supportsPosix(file)) {
            Files.setPosixFilePermissions(file, OWNER_ONLY_FILE);
        }
        // On Windows the file inherits the owner-only ACL of the directory it is created in.
    }

    static boolean supportsPosix(Path path) {
        return Files.getFileAttributeView(path, PosixFileAttributeView.class) != null;
    }

    static boolean isWindows() {
        return System.getProperty("os.name", "").toLowerCase(java.util.Locale.ROOT).contains("windows");
    }

    /**
     * This JVM's process id.
     *
     * <p>{@code ProcessHandle.current().pid()} is Java 9; this artifact targets 8, so the id comes
     * from the {@code <pid>@<host>} shape of the runtime MXBean name — the standard Java 8 answer,
     * and stable on every JVM PlatynUI targets.
     */
    static long currentPid() {
        String name = java.lang.management.ManagementFactory.getRuntimeMXBean().getName();
        int at = name.indexOf('@');
        String pid = at > 0 ? name.substring(0, at) : name;
        try {
            return Long.parseLong(pid.trim());
        } catch (NumberFormatException e) {
            throw new IllegalStateException("cannot determine this JVM's process id from '" + name + "'", e);
        }
    }
}
