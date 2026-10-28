namespace Aftercast.ActCompat;

// Stable bounded ordering observes comparer consistency; it cannot prove arbitrary plugin code is pure or terminating.
internal static class TableOrdering
{
    private sealed class OrderingFailure(string code) : Exception(code);
    private const int MaximumEvaluations = 500_000, MaximumObservedPairs = 262_144;
    internal static int[] Order(int count, Func<int, int, int> compare, bool descending,
        Action<Exception> diagnose, out string? error, bool reverseTies = false)
    {
        var original = Enumerable.Range(0, count).ToArray();
        error = null;
        try
        {
            var observations = new Dictionary<(int Left, int Right), int>();
            var evaluations = 0;
            int Evaluate(int left, int right)
            {
                if (++evaluations > MaximumEvaluations) throw new OrderingFailure("comparison-budget");
                return Math.Sign(compare(left, right));
            }
            int CheckedSign(int left, int right)
            {
                var sign = Evaluate(left, right);
                if (sign != -Evaluate(right, left) || left == right && sign != 0)
                    throw new OrderingFailure("inconsistent-comparison");
                if (left != right)
                {
                    var key = left < right ? (left, right) : (right, left);
                    var canonical = left < right ? sign : -sign;
                    if (observations.TryGetValue(key, out var previous))
                    {
                        if (previous != canonical) throw new OrderingFailure("inconsistent-comparison");
                    }
                    else
                    {
                        if (observations.Count == MaximumObservedPairs) throw new OrderingFailure("comparison-budget");
                        observations.Add(key, canonical);
                    }
                }
                return sign;
            }
            int Compare(int left, int right)
            {
                var sign = CheckedSign(left, right);
                return sign == 0 ? left.CompareTo(right) : descending ? -sign : sign;
            }
            for (var index = 0; index < count; index++) Compare(index, index);
            var order = original.ToArray();
            if (reverseTies)
            {
                // ACT uses List.Sort and then Reverse, including its runtime's equal-key ordering.
                Array.Sort(order, (left, right) => CheckedSign(left, right));
                if (descending) Array.Reverse(order);
            }
            else
            {
                var scratch = new int[count];
                for (var width = 1; width < count; width *= 2)
                {
                    for (var start = 0; start < count; start += width * 2)
                    {
                        var middle = Math.Min(start + width, count);
                        var end = Math.Min(start + width * 2, count);
                        var left = start; var right = middle;
                        for (var destination = start; destination < end; destination++)
                            scratch[destination] = left < middle && (right == end || Compare(order[left], order[right]) <= 0)
                                ? order[left++] : order[right++];
                    }
                    (order, scratch) = (scratch, order);
                }
            }
            // Exhaust small tables; on larger tables validate observed pairs and every adjacent result.
            if (count <= 64)
                for (var left = 0; left < count; left++)
                    for (var right = left + 1; right < count; right++) Compare(left, right);
            for (var index = 1; index < count; index++) Compare(order[index - 1], order[index]);
            var ranks = new int[count];
            for (var index = 0; index < count; index++) ranks[order[index]] = index;
            var ties = original.ToArray();
            int Find(int value)
            {
                while (ties[value] != value) { ties[value] = ties[ties[value]]; value = ties[value]; }
                return value;
            }
            foreach (var pair in observations)
            {
                Compare(pair.Key.Left, pair.Key.Right);
                if (pair.Value == 0) ties[Find(pair.Key.Left)] = Find(pair.Key.Right);
            }
            foreach (var pair in observations)
            {
                if (pair.Value == 0) continue;
                var sign = descending ? -pair.Value : pair.Value;
                if (Find(pair.Key.Left) == Find(pair.Key.Right) || Math.Sign(ranks[pair.Key.Left] - ranks[pair.Key.Right]) != sign)
                    throw new OrderingFailure("inconsistent-comparison");
            }
            return order;
        }
        catch (TableQueryInvalidatedException) { throw; }
        catch (Exception failure)
        {
            if (failure is InvalidOperationException { InnerException: TableQueryInvalidatedException invalidated }) throw invalidated;
            var reason = failure is InvalidOperationException { InnerException: OrderingFailure bounded } ? bounded : failure;
            error = reason is OrderingFailure ? reason.Message : "sort-callback-error";
            diagnose(failure);
            return original;
        }
    }
}
