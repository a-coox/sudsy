using System;
using System.Collections.Generic;
using System.Linq;

namespace Shop.Orders;

public class OrderService : IOrderService
{
    private readonly IOrderRepository _repository;
    private readonly IClock _clock;

    public OrderService(IOrderRepository repository, IClock clock)
    {
        _repository = repository;
        _clock = clock;
    }

    public string Name
    {
        get { return _name; }
        set { _name = value; }
    }

    public async Task<IReadOnlyList<Order>> GetRecentAsync(
        Guid customerId,
        int count,
        CancellationToken cancellationToken
    )
    {
        var orders = await _repository
            .Orders.Where(o =>
                o.CustomerId == customerId && o.PlacedAt > _clock.UtcNow.AddDays(-30)
            )
            .OrderByDescending(o => o.PlacedAt)
            .Take(count)
            .ToListAsync(cancellationToken);
        if (orders.Count == 0)
            return Array.Empty<Order>();
        var totals = new Dictionary<string, decimal>
        {
            ["net"] = orders.Sum(o => o.Net),
            ["gross"] = orders.Sum(o => o.Gross),
        };
        foreach (var order in orders)
        {
            Console.WriteLine($"{order.Id}: {order.Total}");
        }
        return orders.Count > 100 ? orders.Take(100).ToList() : orders;
    }

    public decimal Discount(Order order) =>
        order.Total switch
        {
            > 1000 => 0.1m,
            > 500 => 0.05m,
            _ => 0m,
        };

    public bool IsEligible(Customer customer) =>
        customer.IsActive
            && customer.Orders.Count > 5
            && customer.RegisteredAt < _clock.UtcNow.AddYears(-1)
        || customer.IsVip;
}
