package no.experis.orders;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.math.BigDecimal;
import java.util.UUID;
import org.junit.jupiter.api.Test;

/** The order feature, checked against the fixed API of the order plans. The model never sees this file. */
class OrderHiddenTest {
    private static void assertTotal(String unitPrice, int quantity, String expected) {
        Order order = Order.place("CUST-001", "PROD-A", new BigDecimal(unitPrice), quantity);
        assertEquals(0, new BigDecimal(expected).compareTo(order.total()),
                unitPrice + " x " + quantity + " should total " + expected + " but was " + order.total());
    }

    @Test
    void anOrderForOneProductTotalsItsPrice() {
        assertTotal("100.00", 1, "100.00");
    }

    @Test
    void anAmountBelowFiveHundredGetsNoDiscount() {
        assertTotal("60.00", 8, "480.00");
        assertTotal("99.99", 5, "499.95");
    }

    @Test
    void anAmountOfFiveHundredOrMoreGetsTenPercentOff() {
        assertTotal("60.00", 9, "486.00");
        assertTotal("60.00", 10, "540.00");
        assertTotal("100.00", 5, "450.00");
    }

    @Test
    void aQuantityThatIsNotPositiveIsRejectedWithTheMessage() {
        for (int quantity : new int[] {0, -1}) {
            IllegalArgumentException e = assertThrows(IllegalArgumentException.class,
                    () -> Order.place("CUST-003", "PROD-C", new BigDecimal("25.00"), quantity));
            assertEquals("Quantity must be positive", e.getMessage());
        }
    }

    @Test
    void anOrderKeepsWhoOrderedWhatAndGetsItsOwnId() {
        Order a = Order.place("CUST-001", "PROD-A", new BigDecimal("100.00"), 2);
        Order b = Order.place("CUST-001", "PROD-A", new BigDecimal("100.00"), 2);
        assertEquals("CUST-001", a.customerId());
        assertEquals("PROD-A", a.productId());
        assertEquals(2, a.quantity());
        assertNotEquals(a.id(), b.id());
    }

    @Test
    void aPlacedOrderCanBeLookedUpByItsId() {
        OrderService service = new OrderService();
        Order placed = service.placeOrder("CUST-001", "PROD-A", new BigDecimal("100.00"), 1);
        Order found = service.findById(placed.id()).orElseThrow();
        assertEquals("CUST-001", found.customerId());
        assertEquals(0, new BigDecimal("100.00").compareTo(found.total()));
    }

    @Test
    void anUnknownIdGivesAnEmptyResult() {
        assertFalse(new OrderService().findById(UUID.randomUUID()).isPresent());
    }

    @Test
    void placingAnOrderThroughTheServiceRejectsAQuantityThatIsNotPositive() {
        OrderService service = new OrderService();
        assertThrows(IllegalArgumentException.class,
                () -> service.placeOrder("CUST-003", "PROD-C", new BigDecimal("25.00"), 0));
    }
}
