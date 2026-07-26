package platynui.agent;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.Writer;
import java.io.OutputStreamWriter;
import java.net.Socket;
import java.net.SocketTimeoutException;
import java.nio.charset.Charset;
import java.util.Collections;
import java.util.Map;

/**
 * One client connection: newline-delimited JSON-RPC 2.0 over the loopback socket.
 *
 * <p>Newline-delimited rather than length-prefixed because the frames are small and a human can
 * read a capture; UTF-8 throughout, and a frame therefore never contains a raw newline (the JSON
 * writer escapes them).
 *
 * <p>Everything except the handshake is closed to an unauthenticated caller: loopback TCP is
 * reachable by <em>any</em> local user, so the socket proves nothing on its own — the token from
 * the owner-only handshake file does.
 */
final class RpcSession {

    private static final Charset UTF_8 = Charset.forName("UTF-8");

    private final RpcServer server;
    private final Socket socket;
    private final Map<String, RpcMethod> methods;
    private final long id;
    private final Object writeLock = new Object();

    private Writer writer;
    private volatile boolean authenticated;
    private volatile boolean closed;

    RpcSession(RpcServer server, Socket socket, Map<String, RpcMethod> methods, long id) {
        this.server = server;
        this.socket = socket;
        this.methods = methods;
        this.id = id;
    }

    long id() {
        return id;
    }

    boolean isAuthenticated() {
        return authenticated;
    }

    /** Called by the handshake method once the token and version have checked out. */
    void markAuthenticated() {
        authenticated = true;
        RpcServer.relaxTimeout(socket);
    }

    String expectedToken() {
        return server.token();
    }

    void run() {
        try {
            OutputStream output = socket.getOutputStream();
            synchronized (writeLock) {
                writer = new OutputStreamWriter(output, UTF_8);
            }
            BufferedReader reader = new BufferedReader(new InputStreamReader(socket.getInputStream(), UTF_8));
            String line;
            while ((line = reader.readLine()) != null) {
                if (closed) {
                    return;
                }
                if (line.trim().isEmpty()) {
                    continue;
                }
                if (!handle(line)) {
                    return;
                }
            }
        } catch (SocketTimeoutException e) {
            AgentLog.debug("session " + id + " dropped: silent before the handshake");
        } catch (IOException e) {
            AgentLog.debug("session " + id + " ended: " + e);
        } finally {
            close();
        }
    }

    /**
     * Handles one frame.
     *
     * @return {@code false} if the connection must be closed
     */
    private boolean handle(String line) {
        Object parsed;
        try {
            parsed = Json.parse(line);
        } catch (Json.SyntaxException e) {
            sendError(null, RpcException.PARSE_ERROR, e.getMessage());
            return true;
        }
        Map<String, Object> request = Json.asObject(parsed);
        if (request == null) {
            sendError(null, RpcException.INVALID_REQUEST, "a request must be a JSON object");
            return true;
        }
        Object requestId = request.get("id");
        String method = Json.asString(request.get("method"));
        if (method == null) {
            sendError(requestId, RpcException.INVALID_REQUEST, "missing 'method'");
            return true;
        }
        Map<String, Object> params = Json.asObject(request.get("params"));
        if (params == null) {
            params = Collections.emptyMap();
        }

        RpcMethod handler = methods.get(method);
        try {
            if (handler == null) {
                throw new RpcException(RpcException.METHOD_NOT_FOUND, "unknown method '" + method + "'");
            }
            if (!authenticated && !handler.allowedBeforeHandshake()) {
                throw new RpcException(RpcException.UNAUTHENTICATED, "handshake required before '" + method + "'", true);
            }
            Object result = handler.invoke(this, params);
            if (requestId != null) {
                sendResult(requestId, result);
            }
            return true;
        } catch (RpcException e) {
            if (requestId != null) {
                sendError(requestId, e.code(), e.getMessage());
            }
            return !e.isFatal();
        } catch (RuntimeException e) {
            // A bug in a method handler must not take the agent's listener with it — the
            // target application would keep running with a silently dead agent.
            AgentLog.error("method '" + method + "' failed", e);
            if (requestId != null) {
                sendError(requestId, RpcException.INTERNAL_ERROR, String.valueOf(e));
            }
            return true;
        }
    }

    private void sendResult(Object requestId, Object result) {
        Map<String, Object> response = Json.newObject();
        response.put("jsonrpc", "2.0");
        response.put("id", requestId);
        response.put("result", result);
        send(response);
    }

    private void sendError(Object requestId, int code, String message) {
        Map<String, Object> error = Json.newObject();
        error.put("code", Long.valueOf(code));
        error.put("message", message == null ? "" : message);
        Map<String, Object> response = Json.newObject();
        response.put("jsonrpc", "2.0");
        response.put("id", requestId);
        response.put("error", error);
        send(response);
    }

    /** Sends a server-initiated notification; silently skipped before the handshake. */
    void notifyClient(String method, Map<String, Object> params) {
        if (!authenticated) {
            return;
        }
        Map<String, Object> notification = Json.newObject();
        notification.put("jsonrpc", "2.0");
        notification.put("method", method);
        notification.put("params", params);
        send(notification);
    }

    private void send(Map<String, Object> message) {
        String frame = Json.write(message);
        synchronized (writeLock) {
            if (writer == null || closed) {
                return;
            }
            try {
                writer.write(frame);
                writer.write('\n');
                writer.flush();
            } catch (IOException e) {
                AgentLog.debug("session " + id + " write failed: " + e);
                closed = true;
            }
        }
    }

    void close() {
        closed = true;
        RpcServer.closeQuietly(socket);
    }
}
