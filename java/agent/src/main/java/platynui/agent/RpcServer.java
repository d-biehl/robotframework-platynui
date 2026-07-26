package platynui.agent;

import java.io.IOException;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.Collections;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

/**
 * The agent's data plane: a loopback JSON-RPC server, one thread per connection.
 *
 * <p>Loopback TCP is forced, not merely chosen (design decision 1a): the agent must run on Java 8
 * targets, where Unix domain sockets do not exist yet (JEP 380 is Java 16) and a named-pipe
 * <em>server</em> is impossible in pure Java. The port is OS-chosen ({@code :0}), so concurrent
 * target JVMs cannot collide, and it is published through the handshake file rather than
 * configured.
 *
 * <p><strong>Multiple concurrent connections are the point</strong>, not a bonus: the Inspector and
 * a test run are separate PlatynUI processes and must not lock each other out (design decision 7).
 * Serialisation happens where it has to — on the toolkit thread — not at the socket.
 */
final class RpcServer {

    /** How long an unauthenticated connection may stay silent before it is dropped. */
    private static final int HANDSHAKE_TIMEOUT_MS = 10_000;

    private final Map<String, RpcMethod> methods;
    private final String token;
    private final Set<RpcSession> sessions =
            Collections.newSetFromMap(new ConcurrentHashMap<RpcSession, Boolean>());
    private final AtomicLong sessionIds = new AtomicLong();

    private ServerSocket serverSocket;
    private volatile boolean running;

    RpcServer(Map<String, RpcMethod> methods, String token) {
        this.methods = methods;
        this.token = token;
    }

    /**
     * Binds the loopback port and starts accepting.
     *
     * @return the bound port
     */
    int start() throws IOException {
        serverSocket = new ServerSocket(0, 16, InetAddress.getLoopbackAddress());
        running = true;
        Thread acceptor = new Thread(new Runnable() {
            @Override
            public void run() {
                acceptLoop();
            }
        }, "platynui-agent-accept");
        // Daemon throughout: the agent must never be the reason a target application
        // refuses to exit.
        acceptor.setDaemon(true);
        acceptor.start();
        return serverSocket.getLocalPort();
    }

    String token() {
        return token;
    }

    private void acceptLoop() {
        while (running) {
            Socket socket;
            try {
                socket = serverSocket.accept();
            } catch (IOException e) {
                if (running) {
                    AgentLog.error("accept failed; the agent stops listening", e);
                }
                return;
            }
            try {
                socket.setTcpNoDelay(true);
                socket.setSoTimeout(HANDSHAKE_TIMEOUT_MS);
            } catch (IOException e) {
                closeQuietly(socket);
                continue;
            }
            final RpcSession session = new RpcSession(this, socket, methods, sessionIds.incrementAndGet());
            sessions.add(session);
            Thread worker = new Thread(new Runnable() {
                @Override
                public void run() {
                    try {
                        session.run();
                    } finally {
                        sessions.remove(session);
                    }
                }
            }, "platynui-agent-session-" + session.id());
            worker.setDaemon(true);
            worker.start();
        }
    }

    /**
     * Pushes a server-initiated notification to every authenticated connection.
     *
     * <p>The notification frame — a JSON-RPC message without an {@code id} — is part of the wire
     * from day one so later event capabilities need no protocol break. Version 1 uses it only for
     * the UI-generation counter.
     */
    void broadcast(String method, Map<String, Object> params) {
        for (RpcSession session : sessions) {
            session.notifyClient(method, params);
        }
    }

    /** Clears the connection's soft timeout once it has proven itself. */
    static void relaxTimeout(Socket socket) {
        try {
            socket.setSoTimeout(0);
        } catch (IOException e) {
            AgentLog.debug("could not clear the socket timeout: " + e);
        }
    }

    void stop() {
        running = false;
        try {
            if (serverSocket != null) {
                serverSocket.close();
            }
        } catch (IOException e) {
            AgentLog.debug("closing the server socket failed: " + e);
        }
        for (RpcSession session : sessions) {
            session.close();
        }
    }

    static void closeQuietly(Socket socket) {
        try {
            socket.close();
        } catch (IOException e) {
            AgentLog.debug("closing a session socket failed: " + e);
        }
    }
}
