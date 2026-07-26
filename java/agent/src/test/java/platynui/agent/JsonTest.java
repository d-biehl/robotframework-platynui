package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

/**
 * The JSON layer is hand-rolled (the agent ships no dependencies), so it carries its own tests. The
 * cases that matter are the ones a real application produces: non-ASCII accessible names, quotes
 * and newlines in labels, and numbers that must survive a round trip as integers.
 */
class JsonTest {

    @Test
    void integral_numbers_survive_as_integers() throws Exception {
        // Element ids and ports must not come back as 1.0E9.
        Object parsed = Json.parse("{\"id\":9007199254740993,\"port\":51234}");
        Map<String, Object> object = Json.asObject(parsed);
        assertEquals(Long.valueOf(9007199254740993L), object.get("id"));
        assertEquals(Long.valueOf(51234L), object.get("port"));
    }

    @Test
    void fractional_numbers_stay_fractional() throws Exception {
        Map<String, Object> object = Json.asObject(Json.parse("{\"x\":1.5,\"y\":-2.25e2}"));
        assertEquals(Double.valueOf(1.5), object.get("x"));
        assertEquals(Double.valueOf(-225.0), object.get("y"));
    }

    @Test
    void text_round_trips_through_escapes_and_non_ascii() throws Exception {
        String awkward = "Datei \"öffnen\"\n\tZeile\\Pfad \u0001";
        Map<String, Object> written = Json.newObject();
        written.put("name", awkward);
        String encoded = Json.write(written);
        assertTrue(encoded.contains("\\u0001"), "control characters must be escaped: " + encoded);
        assertTrue(encoded.contains("öffnen"), "the wire is UTF-8; non-ASCII goes out verbatim: " + encoded);
        assertEquals(awkward, Json.asObject(Json.parse(encoded)).get("name"));
    }

    @Test
    void unicode_escapes_are_read_back() throws Exception {
        assertEquals("ö", Json.asObject(Json.parse("{\"a\":\"\\u00f6\"}")).get("a"));
    }

    @Test
    void nesting_and_empties_work() throws Exception {
        Map<String, Object> object =
                Json.asObject(Json.parse("{\"a\":[],\"b\":{},\"c\":[1,{\"d\":null}],\"e\":true}"));
        assertTrue(((List<?>) object.get("a")).isEmpty());
        assertTrue(Json.asObject(object.get("b")).isEmpty());
        assertEquals(2, ((List<?>) object.get("c")).size());
        assertNull(Json.asObject(((List<?>) object.get("c")).get(1)).get("d"));
        assertEquals(Boolean.TRUE, object.get("e"));
    }

    @Test
    void a_written_object_keeps_its_insertion_order() {
        Map<String, Object> object = Json.newObject();
        object.put("protocol", Long.valueOf(1));
        object.put("pid", Long.valueOf(7));
        object.put("toolkits", Arrays.asList("swing", "javafx"));
        assertEquals("{\"protocol\":1,\"pid\":7,\"toolkits\":[\"swing\",\"javafx\"]}", Json.write(object));
    }

    @Test
    void malformed_input_is_a_syntax_error_rather_than_a_wrong_answer() {
        for (String bad : new String[] {"", "{", "{\"a\"}", "{\"a\":}", "[1,]", "\"unterminated", "{}{}", "\\u12"}) {
            assertThrows(Json.SyntaxException.class, () -> Json.parse(bad), "must reject: " + bad);
        }
    }

    @Test
    void non_finite_numbers_degrade_to_null_rather_than_to_an_unparsable_frame() {
        Map<String, Object> object = Json.newObject();
        object.put("x", Double.valueOf(Double.NaN));
        object.put("y", Double.valueOf(Double.POSITIVE_INFINITY));
        // JSON has no NaN literal: emitting one would produce a frame no
        // conforming parser accepts, breaking the whole connection over one value.
        assertEquals("{\"x\":null,\"y\":null}", Json.write(object));
    }

    @Test
    void a_non_object_value_views_as_null_instead_of_throwing() {
        assertNull(Json.asObject("string"));
        assertNull(Json.asString(Long.valueOf(1)));
    }
}
