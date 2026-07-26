package platynui.agent;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * A minimal JSON reader/writer.
 *
 * <p>Hand-rolled on purpose: the agent runs inside a foreign process, so it brings no dependencies
 * (see the module README). The wire is newline-delimited JSON-RPC 2.0, which needs exactly this
 * much JSON and no object mapping.
 *
 * <p>Values are plain Java objects — {@link Map} (insertion-ordered), {@link List}, {@link String},
 * {@link Long} or {@link Double}, {@link Boolean}, {@code null}. Integral numbers parse to
 * {@code Long} so element ids and ports survive a round trip unrounded.
 */
final class Json {

    private Json() {
        // Static helper.
    }

    /** Thrown for malformed input; the RPC layer turns it into a parse error. */
    static final class SyntaxException extends Exception {

        private static final long serialVersionUID = 1L;

        SyntaxException(String message) {
            super(message);
        }
    }

    // ---------------------------------------------------------------- parsing

    static Object parse(String text) throws SyntaxException {
        Parser parser = new Parser(text);
        parser.skipWhitespace();
        Object value = parser.readValue();
        parser.skipWhitespace();
        if (!parser.atEnd()) {
            throw new SyntaxException("trailing content after the JSON value");
        }
        return value;
    }

    private static final class Parser {

        private final String text;
        private int position;

        Parser(String text) {
            this.text = text;
        }

        boolean atEnd() {
            return position >= text.length();
        }

        void skipWhitespace() {
            while (position < text.length()) {
                char c = text.charAt(position);
                if (c != ' ' && c != '\t' && c != '\n' && c != '\r') {
                    return;
                }
                position++;
            }
        }

        Object readValue() throws SyntaxException {
            if (atEnd()) {
                throw new SyntaxException("unexpected end of input");
            }
            char c = text.charAt(position);
            switch (c) {
                case '{':
                    return readObject();
                case '[':
                    return readArray();
                case '"':
                    return readString();
                case 't':
                    expect("true");
                    return Boolean.TRUE;
                case 'f':
                    expect("false");
                    return Boolean.FALSE;
                case 'n':
                    expect("null");
                    return null;
                default:
                    return readNumber();
            }
        }

        private void expect(String literal) throws SyntaxException {
            if (!text.startsWith(literal, position)) {
                throw new SyntaxException("expected " + literal + " at offset " + position);
            }
            position += literal.length();
        }

        private Map<String, Object> readObject() throws SyntaxException {
            position++; // '{'
            Map<String, Object> object = new LinkedHashMap<String, Object>();
            skipWhitespace();
            if (peek() == '}') {
                position++;
                return object;
            }
            while (true) {
                skipWhitespace();
                if (peek() != '"') {
                    throw new SyntaxException("expected an object key at offset " + position);
                }
                String key = readString();
                skipWhitespace();
                if (peek() != ':') {
                    throw new SyntaxException("expected ':' at offset " + position);
                }
                position++;
                skipWhitespace();
                object.put(key, readValue());
                skipWhitespace();
                char c = peek();
                position++;
                if (c == '}') {
                    return object;
                }
                if (c != ',') {
                    throw new SyntaxException("expected ',' or '}' at offset " + (position - 1));
                }
            }
        }

        private List<Object> readArray() throws SyntaxException {
            position++; // '['
            List<Object> array = new ArrayList<Object>();
            skipWhitespace();
            if (peek() == ']') {
                position++;
                return array;
            }
            while (true) {
                skipWhitespace();
                array.add(readValue());
                skipWhitespace();
                char c = peek();
                position++;
                if (c == ']') {
                    return array;
                }
                if (c != ',') {
                    throw new SyntaxException("expected ',' or ']' at offset " + (position - 1));
                }
            }
        }

        private String readString() throws SyntaxException {
            position++; // opening quote
            StringBuilder builder = new StringBuilder();
            while (true) {
                if (atEnd()) {
                    throw new SyntaxException("unterminated string");
                }
                char c = text.charAt(position++);
                if (c == '"') {
                    return builder.toString();
                }
                if (c != '\\') {
                    builder.append(c);
                    continue;
                }
                if (atEnd()) {
                    throw new SyntaxException("unterminated escape sequence");
                }
                char escape = text.charAt(position++);
                switch (escape) {
                    case '"':
                        builder.append('"');
                        break;
                    case '\\':
                        builder.append('\\');
                        break;
                    case '/':
                        builder.append('/');
                        break;
                    case 'b':
                        builder.append('\b');
                        break;
                    case 'f':
                        builder.append('\f');
                        break;
                    case 'n':
                        builder.append('\n');
                        break;
                    case 'r':
                        builder.append('\r');
                        break;
                    case 't':
                        builder.append('\t');
                        break;
                    case 'u':
                        if (position + 4 > text.length()) {
                            throw new SyntaxException("truncated \\u escape");
                        }
                        try {
                            builder.append((char) Integer.parseInt(text.substring(position, position + 4), 16));
                        } catch (NumberFormatException e) {
                            throw new SyntaxException("malformed \\u escape at offset " + position);
                        }
                        position += 4;
                        break;
                    default:
                        throw new SyntaxException("unknown escape \\" + escape);
                }
            }
        }

        private Object readNumber() throws SyntaxException {
            int start = position;
            boolean integral = true;
            while (position < text.length()) {
                char c = text.charAt(position);
                if (c == '-' || c == '+' || (c >= '0' && c <= '9')) {
                    position++;
                } else if (c == '.' || c == 'e' || c == 'E') {
                    integral = false;
                    position++;
                } else {
                    break;
                }
            }
            String literal = text.substring(start, position);
            if (literal.isEmpty()) {
                throw new SyntaxException("expected a value at offset " + start);
            }
            try {
                if (integral) {
                    return Long.valueOf(literal);
                }
                return Double.valueOf(literal);
            } catch (NumberFormatException e) {
                throw new SyntaxException("malformed number '" + literal + "'");
            }
        }

        private char peek() throws SyntaxException {
            if (atEnd()) {
                throw new SyntaxException("unexpected end of input");
            }
            return text.charAt(position);
        }
    }

    // ---------------------------------------------------------------- writing

    static String write(Object value) {
        StringBuilder builder = new StringBuilder();
        writeValue(builder, value);
        return builder.toString();
    }

    private static void writeValue(StringBuilder builder, Object value) {
        if (value == null) {
            builder.append("null");
        } else if (value instanceof String) {
            writeString(builder, (String) value);
        } else if (value instanceof Boolean || value instanceof Long || value instanceof Integer) {
            builder.append(value);
        } else if (value instanceof Double || value instanceof Float) {
            double d = ((Number) value).doubleValue();
            // JSON has no NaN/Infinity; emitting one would produce a document no
            // conforming parser accepts, so they degrade to null rather than to
            // an unparsable frame.
            builder.append(Double.isNaN(d) || Double.isInfinite(d) ? "null" : Double.toString(d));
        } else if (value instanceof Map) {
            writeObject(builder, asObject(value));
        } else if (value instanceof Iterable) {
            writeArray(builder, (Iterable<?>) value);
        } else {
            writeString(builder, value.toString());
        }
    }

    private static void writeObject(StringBuilder builder, Map<String, Object> object) {
        builder.append('{');
        boolean first = true;
        for (Map.Entry<String, Object> entry : object.entrySet()) {
            if (!first) {
                builder.append(',');
            }
            first = false;
            writeString(builder, entry.getKey());
            builder.append(':');
            writeValue(builder, entry.getValue());
        }
        builder.append('}');
    }

    private static void writeArray(StringBuilder builder, Iterable<?> values) {
        builder.append('[');
        boolean first = true;
        for (Object value : values) {
            if (!first) {
                builder.append(',');
            }
            first = false;
            writeValue(builder, value);
        }
        builder.append(']');
    }

    private static void writeString(StringBuilder builder, String value) {
        builder.append('"');
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            switch (c) {
                case '"':
                    builder.append("\\\"");
                    break;
                case '\\':
                    builder.append("\\\\");
                    break;
                case '\n':
                    builder.append("\\n");
                    break;
                case '\r':
                    builder.append("\\r");
                    break;
                case '\t':
                    builder.append("\\t");
                    break;
                case '\b':
                    builder.append("\\b");
                    break;
                case '\f':
                    builder.append("\\f");
                    break;
                default:
                    // Control characters must be escaped; everything else — including
                    // non-ASCII — goes out verbatim, since the wire is UTF-8. Accessible
                    // names in the applications we read are routinely non-ASCII.
                    if (c < 0x20) {
                        builder.append(String.format("\\u%04x", Integer.valueOf(c)));
                    } else {
                        builder.append(c);
                    }
                    break;
            }
        }
        builder.append('"');
    }

    // ---------------------------------------------------------------- helpers

    static Map<String, Object> newObject() {
        return new LinkedHashMap<String, Object>();
    }

    /**
     * Views a parsed value as a JSON object.
     *
     * @return the map, or {@code null} if the value is not an object
     */
    @SuppressWarnings("unchecked")
    static Map<String, Object> asObject(Object value) {
        return value instanceof Map ? (Map<String, Object>) value : null;
    }

    static String asString(Object value) {
        return value instanceof String ? (String) value : null;
    }
}
