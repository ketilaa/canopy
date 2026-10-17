package no.experis.stock;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

/** The stock feature against the fixed API of the stock plans. The model never sees this file. */
class StockHiddenTest {
    private final StockRepository repository = new InMemoryStockRepository();
    private final StockService service = new StockService(repository);

    @Test
    void receivedStockIsAvailableAndMoreStockAddsToIt() {
        service.receive("SKU-1", 10);
        assertEquals(10, service.available("SKU-1"));
        service.receive("SKU-1", 5);
        assertEquals(15, service.available("SKU-1"));
        assertEquals(0, service.available("SKU-2"));
    }

    @Test
    void reservingTakesUnitsOutOfWhatIsAvailable() {
        service.receive("SKU-1", 10);
        service.reserve("SKU-1", 4);
        assertEquals(6, service.available("SKU-1"));
        service.reserve("SKU-1", 6);
        assertEquals(0, service.available("SKU-1"));
    }

    @Test
    void reservingMoreThanIsAvailableIsRejectedAndChangesNothing() {
        service.receive("SKU-1", 10);
        IllegalStateException e = assertThrows(IllegalStateException.class, () -> service.reserve("SKU-1", 11));
        assertEquals("Insufficient stock", e.getMessage());
        assertEquals(10, service.available("SKU-1"));
    }

    @Test
    void reservingAnUnknownSkuIsRejected() {
        IllegalArgumentException e = assertThrows(IllegalArgumentException.class, () -> service.reserve("SKU-9", 1));
        assertEquals("Unknown SKU", e.getMessage());
    }

    @Test
    void aQuantityThatIsNotPositiveIsRejectedWithTheMessage() {
        service.receive("SKU-1", 10);
        for (int quantity : new int[] {0, -3}) {
            IllegalArgumentException received = assertThrows(IllegalArgumentException.class, () -> service.receive("SKU-1", quantity));
            assertEquals("Quantity must be positive", received.getMessage());
            IllegalArgumentException reserved = assertThrows(IllegalArgumentException.class, () -> service.reserve("SKU-1", quantity));
            assertEquals("Quantity must be positive", reserved.getMessage());
        }
        assertEquals(10, service.available("SKU-1"));
    }

    @Test
    void aStockItemIsImmutable() {
        StockItem item = new StockItem("SKU-1", 10);
        StockItem more = item.receive(5);
        StockItem less = item.reserve(4);
        assertEquals(10, item.onHand());
        assertEquals(15, more.onHand());
        assertEquals(6, less.onHand());
        assertEquals("SKU-1", less.sku());
        IllegalStateException e = assertThrows(IllegalStateException.class, () -> item.reserve(11));
        assertEquals("Insufficient stock", e.getMessage());
    }

    @Test
    void theRepositoryKeepsWhatWasSavedAndKnowsWhatItDoesNot() {
        assertTrue(repository.findBySku("SKU-1").isEmpty());
        repository.save(new StockItem("SKU-1", 3));
        assertEquals(3, repository.findBySku("SKU-1").orElseThrow().onHand());
        repository.save(new StockItem("SKU-1", 7));
        assertEquals(7, repository.findBySku("SKU-1").orElseThrow().onHand());
    }
}
