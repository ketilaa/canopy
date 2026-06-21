package com.example.orders;

import com.example.common.BaseService;
import com.example.common.Repository;

public class OrderService extends BaseService implements Repository {

    public Order placeOrder(String customerId, String item) {
        validateInput(customerId);
        return new Order(customerId, item);
    }

    public void cancelOrder(String orderId) {
        notifyCustomer(orderId);
    }
}

public interface OrderRepository {
    Order findById(String id);
}

public enum OrderStatus {
    PENDING, CONFIRMED, CANCELLED
}
