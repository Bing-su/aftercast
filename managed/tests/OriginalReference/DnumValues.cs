using System.Reflection;
using System.Reflection.Emit;
using System.Text.Json;

internal static class DnumValues
{
    internal static int Run(Assembly act)
    {
        var unexpectedFailures = 0;
        var type = act.GetType("Advanced_Combat_Tracker.Dnum", throwOnError: true)!;
        object New(long number, string? label = null) => label is null
            ? Activator.CreateInstance(type, number)! : Activator.CreateInstance(type, number, label)!;
        object? Call(object value, string method, Type[] signature, params object?[] arguments) =>
            type.GetMethod(method, signature)!.Invoke(value, arguments);
        object? Operator(string name, params object?[] arguments) =>
            type.GetMethod(name, BindingFlags.Public | BindingFlags.Static, [type, type])!.Invoke(null, arguments);
        object? Describe(object? value) => value is not null && type.IsInstanceOfType(value)
            ? new { number = type.GetProperty("Number")!.GetValue(value), label = type.GetField("damageString", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(value) }
            : value;
        void Observe(string name, Func<object?> action, Type? expectedException = null)
        {
            try
            {
                Console.WriteLine(JsonSerializer.Serialize(new { observation = name, result = Describe(action()) }));
                if (expectedException is not null) unexpectedFailures++;
            }
            catch (Exception error)
            {
                while (error is TargetInvocationException && error.InnerException is not null) error = error.InnerException;
                Console.WriteLine(JsonSerializer.Serialize(new { observation = name, exception = error.GetType().FullName, message = error.Message }));
                if (error.GetType() != expectedException) unexpectedFailures++;
            }
        }
        foreach (var number in new long[] { long.MinValue, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0, 1, 1234567890123, long.MaxValue })
        {
            var value = New(number);
            Observe($"ToString {number}", () => Call(value, "ToString", []));
            Observe($"ToString(false) {number}", () => Call(value, "ToString", [typeof(bool)], false));
            Observe($"ToString(true) {number}", () => Call(value, "ToString", [typeof(bool)], true));
            Observe($"hash follows ToString(false) {number}", () => value.GetHashCode() == ((string)Call(value, "ToString", [typeof(bool)], false)!).GetHashCode());
        }
        var first = New(10, "First");
        var second = New(10, "Second");
        Observe("custom ToString", () => Call(first, "ToString", []));
        Observe("custom ToString(false)", () => Call(first, "ToString", [typeof(bool)], false));
        Observe("custom ToString(true)", () => Call(first, "ToString", [typeof(bool)], true));
        var nullLabel = Activator.CreateInstance(type, [10L, null])!;
        Observe("explicit null label ToString", () => Call(nullLabel, "ToString", []));
        Observe("explicit null label DamageString", () => type.GetProperty("DamageString")!.GetValue(nullLabel));
        foreach (var number in new long[] { -9, -1, 0 })
        {
            var custom = New(number, "Custom");
            Observe($"custom {number} ToString", () => Call(custom, "ToString", []));
            Observe($"custom {number} ToString(false)", () => Call(custom, "ToString", [typeof(bool)], false));
            Observe($"custom {number} ToString(true)", () => Call(custom, "ToString", [typeof(bool)], true));
        }
        type.GetProperty("DamageString2")!.SetValue(first, "Secondary");
        Observe("DamageString2 stored", () => type.GetProperty("DamageString2")!.GetValue(first));
        Observe("DamageString2 default formatting", () => Call(first, "ToString", []));
        type.GetProperty("DamageString2")!.SetValue(first, null);
        Observe("Equals null", () => first.Equals(null), typeof(NullReferenceException));
        Observe("Equals string", () => first.Equals("First"), typeof(InvalidCastException));
        Observe("Equals Int64", () => first.Equals(10L), typeof(InvalidCastException));
        Observe("Equals same number different labels", () => first.Equals(second));
        Observe("Equals same number same labels", () => first.Equals(New(10, "First")));
        Observe("Equals different number same labels", () => first.Equals(New(11, "First")));
        Observe("operator == null,null", () => Operator("op_Equality", null, null), typeof(NullReferenceException));
        Observe("operator == null,value", () => Operator("op_Equality", null, first), typeof(NullReferenceException));
        Observe("operator == value,null", () => Operator("op_Equality", first, null), typeof(NullReferenceException));
        Observe("operator != value,null", () => Operator("op_Inequality", first, null), typeof(NullReferenceException));
        Observe("CompareTo null", () => Call(first, "CompareTo", [typeof(object)], [null]), typeof(NullReferenceException));
        Observe("CompareTo Int64", () => Call(first, "CompareTo", [typeof(object)], 10L), typeof(InvalidCastException));
        Observe("CompareTo same number different labels", () => Call(first, "CompareTo", [typeof(object)], second));
        Observe("CompareTo -9 labels Alpha,Omega", () => Call(New(-9, "Alpha"), "CompareTo", [typeof(object)], New(-9, "Omega")));
        foreach (var (left, right) in new (long Left, long Right)[] { (-1, 10), (10, -9), (-9, -10), (0, -11), (long.MaxValue, 1), (long.MinValue, -1) })
            Observe($"addition {left},{right}", () => Operator("op_Addition", New(left, "Left"), New(right, "Right")));
        Observe("addition null,value", () => Operator("op_Addition", null, first), typeof(NullReferenceException));
        var implicitMethod = type.GetMethod("op_Implicit", BindingFlags.Public | BindingFlags.Static, [typeof(long)])!;
        Observe("direct constructor -11", () => New(-11));
        Observe("implicit conversion -11", () => implicitMethod.Invoke(null, [-11L]));

        var originalCulture = System.Globalization.CultureInfo.CurrentCulture;
        var globals = act.GetType("Advanced_Combat_Tracker.ActGlobals", throwOnError: true)!;
        var commas = globals.GetField("mainTableShowCommas")!;
        var originalCommas = commas.GetValue(null);
        try
        {
            foreach (var culture in new[] { "en-US", "fr-FR" })
                foreach (var enabled in new[] { true, false })
                {
                    System.Globalization.CultureInfo.CurrentCulture = System.Globalization.CultureInfo.GetCultureInfo(culture);
                    commas.SetValue(null, enabled);
                    var value = New(1234567);
                    Observe($"culture {culture} commas {enabled} default", () => Call(value, "ToString", []));
                    Observe($"culture {culture} commas {enabled} formatted", () => Call(value, "ToString", [typeof(bool)], false));
                    Observe($"culture {culture} commas {enabled} raw", () => Call(value, "ToString", [typeof(bool)], true));
                }
        }
        finally
        {
            System.Globalization.CultureInfo.CurrentCulture = originalCulture;
            commas.SetValue(null, originalCommas);
        }

        // Derive through the genuine base constructor to test type behavior without uninitialized objects.
        var builder = AssemblyBuilder.DefineDynamicAssembly(new("Aftercast.OriginalReference.Derived"), AssemblyBuilderAccess.Run)
            .DefineDynamicModule("Derived").DefineType("DerivedDnum", TypeAttributes.Public, type);
        var constructor = builder.DefineConstructor(MethodAttributes.Public, CallingConventions.Standard, [typeof(long)]);
        var il = constructor.GetILGenerator();
        il.Emit(OpCodes.Ldarg_0);
        il.Emit(OpCodes.Ldarg_1);
        il.Emit(OpCodes.Call, type.GetConstructor([typeof(long)])!);
        il.Emit(OpCodes.Ret);
        var derived = Activator.CreateInstance(builder.CreateType()!, 10L)!;
        Observe("Equals genuine derived Dnum", () => New(10).Equals(derived));
        return unexpectedFailures;
    }
}
