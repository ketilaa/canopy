package no.experis.orders;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.math.BigDecimal;
import org.junit.jupiter.api.Test;

/**
 * The order feature with the discount boundary reversed from the real story: a net total of exactly
 * 500.00 does NOT get the discount; only a net total strictly MORE than 500.00 does. The normal order
 * story (OrderHiddenTest) asserts the opposite at this exact value (500.00 gets 450.00). The model
 * never sees this file.
 */
class OrderExclusiveHiddenTest {
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
    void anAmountOfExactlyFiveHundredGetsNoDiscountEither() {
        // The reversal: this is the boundary instant itself, and it must NOT get the discount.
        assertTotal("100.00", 5, "500.00");
    }

    @Test
    void anAmountStrictlyMoreThanFiveHundredGetsTenPercentOff() {
        // Just above the boundary: this DOES get the discount (450.09), unlike exactly 500.00 above.
        assertTotal("50.01", 10, "450.09");
        assertTotal("60.00", 9, "486.00");
        assertTotal("60.00", 10, "540.00");
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
        assertEquals(placed.id(), service.findById(placed.id()).orElseThrow().id());
    }
}
