package platynui.agent;

import java.io.IOException;
import java.nio.charset.Charset;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.Map;

/**
 * The handshake file — rendezvous, authentication and discovery in one (design decision 1a).
 *
 * <p>On startup the agent publishes {@code agent-<pid>} in the owner-only directory
 * {@link AgentPaths} defines; on shutdown it removes it. One mechanism answers four questions at
 * once: which port the agent bound (the OS picks it, so concurrent target JVMs cannot collide),
 * what token a client must present (loopback TCP alone is connectable by any local user), which
 * toolkits are live, and — by merely existing — whether this JVM has an agent at all.
 *
 * <p>The token is deliberately <em>not</em> a {@code -javaagent} argument: {@code /proc/<pid>/cmdline}
 * is world-readable on Linux. The agent generates it, and only the owner-only file carries it.
 *
 * <p>Writes are atomic (temp file in the same directory, then a move), so a client never reads a
 * half-written file.
 */
final class HandshakeFile {

    /** Wire-format version of the handshake file and the RPC protocol on top of it. */
    static final long PROTOCOL_VERSION = 1L;

    private static final Charset UTF_8 = Charset.forName("UTF-8");

    private final Path path;
    private final long pid;
    private final int port;
    private final String token;
    private final String agentVersion;

    private List<String> toolkits;

    HandshakeFile(Path directory, long pid, int port, String token, String agentVersion, List<String> toolkits) {
        this.path = AgentPaths.handshakeFile(directory, pid);
        this.pid = pid;
        this.port = port;
        this.token = token;
        this.agentVersion = agentVersion;
        this.toolkits = toolkits;
    }

    Path path() {
        return path;
    }

    /** Writes (or rewrites) the file with the current contents. */
    synchronized void publish() throws IOException {
        Map<String, Object> content = Json.newObject();
        content.put("protocol", Long.valueOf(PROTOCOL_VERSION));
        content.put("pid", Long.valueOf(pid));
        content.put("port", Long.valueOf(port));
        content.put("token", token);
        content.put("toolkits", toolkits);
        content.put("agentVersion", agentVersion);

        Path directory = path.getParent();
        Path temporary = Files.createTempFile(directory, "agent-", ".tmp");
        try {
            AgentPaths.hardenFile(temporary);
            Files.write(temporary, (Json.write(content) + "\n").getBytes(UTF_8));
            try {
                Files.move(temporary, path, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            } catch (java.nio.file.AtomicMoveNotSupportedException e) {
                Files.move(temporary, path, StandardCopyOption.REPLACE_EXISTING);
            }
        } finally {
            Files.deleteIfExists(temporary);
        }
    }

    /**
     * Republishes the file if the toolkit set changed.
     *
     * @return {@code true} if the file was rewritten
     */
    synchronized boolean updateToolkits(List<String> detected) throws IOException {
        if (toolkits.equals(detected)) {
            return false;
        }
        toolkits = detected;
        publish();
        return true;
    }

    synchronized List<String> toolkits() {
        return toolkits;
    }

    /**
     * Removes the file.
     *
     * <p>Best effort by nature — a killed JVM never gets here, which is why a client must treat a
     * file whose pid no longer runs as stale rather than trusting its presence.
     */
    synchronized void remove() {
        try {
            Files.deleteIfExists(path);
        } catch (IOException e) {
            AgentLog.debug("could not remove the handshake file " + path + ": " + e);
        }
    }
}
