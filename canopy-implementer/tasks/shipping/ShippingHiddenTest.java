package no.experis.shipping;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.math.BigDecimal;
import org.junit.jupiter.api.Test;

/** The shipping feature against the fixed API of the shipping plans. The model never sees this file. */
class ShippingHiddenTest {
    private final ShippingCalculator calculator = new ShippingCalculator();

    private void assertCost(String expected, Zone zone, int grams, String orderValue) {
        BigDecimal cost = calculator.cost(zone, grams, new BigDecimal(orderValue));
        assertEquals(0, new BigDecimal(expected).compareTo(cost), zone + " " + grams + " g worth " + orderValue + " should cost " + expected + " but was " + cost);
    }

    @Test
    void theBasePriceDependsOnTheZone() {
        assertCost("4.00", Zone.DOMESTIC, 1000, "50.00");
        assertCost("9.00", Zone.EUROPE, 1000, "50.00");
        assertCost("15.00", Zone.WORLD, 1000, "50.00");
        assertEquals(0, new BigDecimal("4.00").compareTo(Zone.DOMESTIC.basePrice()));
        assertEquals(0, new BigDecimal("15.00").compareTo(Zone.WORLD.basePrice()));
    }

    @Test
    void eachStartedHalfKiloAboveAKiloAddsOneAndAHalf() {
        assertCost("4.00", Zone.DOMESTIC, 1, "50.00");
        assertCost("4.00", Zone.DOMESTIC, 1000, "50.00");
        assertCost("5.50", Zone.DOMESTIC, 1001, "50.00");
        assertCost("5.50", Zone.DOMESTIC, 1500, "50.00");
        assertCost("7.00", Zone.DOMESTIC, 1501, "50.00");
        assertCost("12.00", Zone.EUROPE, 2000, "50.00");
        assertCost("91.00", Zone.DOMESTIC, 30000, "50.00");
        assertCost("102.00", Zone.WORLD, 30000, "50.00");
    }

    @Test
    void aWeightOutsideOneToThirtyThousandGramsIsRejectedWithTheMessage() {
        for (int grams : new int[] {0, -5, 30001}) {
            IllegalArgumentException e = assertThrows(IllegalArgumentException.class,
                    () -> calculator.cost(Zone.DOMESTIC, grams, new BigDecimal("50.00")));
            assertEquals("Weight must be between 1 and 30000 grams", e.getMessage());
        }
    }

    @Test
    void domesticOrdersWorthAHundredOrMoreShipFree() {
        assertCost("0.00", Zone.DOMESTIC, 2000, "100.00");
        assertCost("0.00", Zone.DOMESTIC, 30000, "250.00");
        assertCost("7.00", Zone.DOMESTIC, 1501, "99.99");
    }

    @Test
    void otherZonesNeverShipFree() {
        assertCost("9.00", Zone.EUROPE, 1000, "500.00");
        assertCost("15.00", Zone.WORLD, 1000, "500.00");
    }
}
