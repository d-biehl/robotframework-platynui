package platynui.agent;

import java.util.Map;

/** One callable method on the agent's wire. */
interface RpcMethod {

    /**
     * @param session the calling connection — carries the authentication state
     * @param params the request's {@code params} object, never {@code null} (empty if omitted)
     * @return the JSON-ready result value
     * @throws RpcException to report a JSON-RPC error instead of a result
     */
    Object invoke(RpcSession session, Map<String, Object> params) throws RpcException;

    /**
     * Whether this method may be called before the handshake.
     *
     * <p>Only the handshake itself may; everything else is closed to an unauthenticated caller,
     * because loopback TCP is reachable by any local user.
     */
    boolean allowedBeforeHandshake();
}
