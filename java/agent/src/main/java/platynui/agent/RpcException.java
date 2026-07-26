package platynui.agent;

/** A failure that becomes a JSON-RPC error object on the wire. */
final class RpcException extends Exception {

    private static final long serialVersionUID = 1L;

    // JSON-RPC 2.0 reserved codes.
    static final int PARSE_ERROR = -32700;
    static final int INVALID_REQUEST = -32600;
    static final int METHOD_NOT_FOUND = -32601;
    static final int INVALID_PARAMS = -32602;
    static final int INTERNAL_ERROR = -32603;

    // PlatynUI codes, inside the -32000..-32099 range JSON-RPC reserves for the server.
    /** The connection has not completed the handshake, or presented a wrong token. */
    static final int UNAUTHENTICATED = -32001;
    /** Client and agent versions differ; the connection is aborted (design decision 1a). */
    static final int VERSION_MISMATCH = -32002;
    /** The call hit the agent-side deadline and was abandoned (design decision 7). */
    static final int DEADLINE_EXCEEDED = -32003;

    private final int code;

    /** Whether the connection must be closed after reporting this error. */
    private final boolean fatal;

    RpcException(int code, String message) {
        this(code, message, false);
    }

    RpcException(int code, String message, boolean fatal) {
        super(message);
        this.code = code;
        this.fatal = fatal;
    }

    int code() {
        return code;
    }

    boolean isFatal() {
        return fatal;
    }
}
