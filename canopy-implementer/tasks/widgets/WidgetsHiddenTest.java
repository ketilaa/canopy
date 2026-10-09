package com.example.widgets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

import java.util.Map;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.web.client.TestRestTemplate;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;

/** Exercises the real application over HTTP. Only the contract in the story is used: no class names. */
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
class WidgetsHiddenTest {
    @Autowired
    private TestRestTemplate http;

    private ResponseEntity<Map> post(Object body) {
        return http.postForEntity("/widgets", body, Map.class);
    }

    @Test
    void aValidNameCreatesAWidgetWithAGeneratedId() {
        ResponseEntity<Map> response = post(Map.of("name", "Alpha"));
        assertEquals(HttpStatus.CREATED, response.getStatusCode());
        Map<?, ?> body = response.getBody();
        assertNotNull(body);
        assertEquals("Alpha", body.get("name"));
        assertNotNull(UUID.fromString(String.valueOf(body.get("id"))), "the id is a UUID");
    }

    @Test
    void twoWidgetsGetDifferentIds() {
        Object first = post(Map.of("name", "Alpha")).getBody().get("id");
        Object second = post(Map.of("name", "Alpha")).getBody().get("id");
        assertNotEquals(first, second);
    }

    @Test
    void aNameOfExactlyFiftyCharactersIsAccepted() {
        assertEquals(HttpStatus.CREATED, post(Map.of("name", "x".repeat(50))).getStatusCode());
    }

    @Test
    void aNameOfFiftyOneCharactersIsRejected() {
        assertEquals(HttpStatus.BAD_REQUEST, post(Map.of("name", "x".repeat(51))).getStatusCode());
    }

    @Test
    void aBlankNameIsRejected() {
        assertEquals(HttpStatus.BAD_REQUEST, post(Map.of("name", "")).getStatusCode());
        assertEquals(HttpStatus.BAD_REQUEST, post(Map.of("name", "   ")).getStatusCode());
    }

    @Test
    void aMissingNameIsRejected() {
        assertEquals(HttpStatus.BAD_REQUEST, post(Map.of()).getStatusCode());
    }
}
