using System.Diagnostics;
using System.Text.RegularExpressions;
using System.Text;

namespace Aftercast.ActCompat;

// A synthetic label, never a real DSN or connection string. Rules only rewrite
// parameterized statement templates; values are deliberately not interpolated.
public sealed record OdbcRewriteRule(string ConnectionPattern, string Find, string Replacement);
public static class OdbcRewrite
{
    public const string SyntheticConnection = "offline-preview";
    public static OdbcPreview[] Apply(OdbcPreview[] plans, OdbcRewriteRule[] rules)
    {
        ArgumentNullException.ThrowIfNull(plans); ArgumentNullException.ThrowIfNull(rules);
        if (rules.Length > 16) throw new ArgumentException("At most 16 offline SQL rules");
        var limit = TimeSpan.FromMilliseconds(50);
        var compiled = rules.Select(rule => {
            if (rule is null || rule.ConnectionPattern is null || rule.Find is null || rule.Replacement is null ||
                rule.ConnectionPattern.Length > 512 || rule.Find.Length is 0 or > 512 || rule.Replacement.Length > 1024 ||
                rule.ConnectionPattern.Contains('\0') || rule.Find.Contains('\0') || rule.Replacement.Contains('\0'))
                throw new ArgumentException("Invalid offline SQL rule");
            return (Connection:new Regex(rule.ConnectionPattern, RegexOptions.Compiled, limit), Find:new Regex(rule.Find, RegexOptions.Compiled, limit), rule.Replacement);
        }).ToArray();
        var clock = Stopwatch.StartNew();
        string Rewrite(string input)
        {
            foreach (var rule in compiled)
            {
                if (clock.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Offline SQL rules exceeded two seconds");
                if (!rule.Connection.IsMatch(SyntheticConnection)) continue;
                var builder = new StringBuilder(); var cursor = 0; var matches = 0;
                foreach (Match match in rule.Find.Matches(input))
                {
                    if (++matches > 4096 || clock.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Offline SQL rule match budget exceeded");
                    var expansions = rule.Replacement.Count(value => value == '$');
                    if ((long)expansions * input.Length + rule.Replacement.Length > 512 * 1024) throw new InvalidDataException("SQL replacement expansion exceeds limit");
                    var replacement = match.Result(rule.Replacement);
                    if ((long)builder.Length + match.Index - cursor + replacement.Length > 512 * 1024) throw new InvalidDataException("Rewritten SQL exceeds limit");
                    builder.Append(input.AsSpan(cursor, match.Index-cursor)); builder.Append(replacement); cursor=match.Index+match.Length;
                }
                if ((long)builder.Length+input.Length-cursor > 512 * 1024) throw new InvalidDataException("Rewritten SQL exceeds limit");
                builder.Append(input.AsSpan(cursor)); input=builder.ToString();
                if (clock.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Offline SQL rules exceeded two seconds");
            }
            return input;
        }
        static string Quoted(string value) => "\"" + value.Replace("\"", "\"\"", StringComparison.Ordinal) + "\"";
        static string Table(string original, string changed, string prefix, string oldTable)
        {
            var originalPrefix=prefix+Quoted(oldTable);
            if (!original.StartsWith(originalPrefix,StringComparison.Ordinal) || !changed.StartsWith(prefix,StringComparison.Ordinal)) throw new InvalidDataException("Unsupported SQL rule: statement structure changed");
            var suffix=original[originalPrefix.Length..];
            if (!changed.EndsWith(suffix,StringComparison.Ordinal) || changed.Length < prefix.Length+suffix.Length) throw new InvalidDataException("Unsupported SQL rule: columns or parameters changed");
            var token=changed[prefix.Length..(changed.Length-suffix.Length)];
            if (token.Length < 3 || token.Length > 2050 || token[0]!='"' || token[^1]!='"') throw new InvalidDataException("Unsupported SQL table identifier");
            var inside=token[1..^1];
            for(var i=0;i<inside.Length;i++)
            {
                if (char.IsControl(inside[i]) || inside[i]=='?') throw new InvalidDataException("Unsupported SQL table identifier");
                if (inside[i]=='"' && (++i>=inside.Length || inside[i]!='"')) throw new InvalidDataException("Invalid SQL identifier quoting");
            }
            return token;
        }
        return plans.Select(plan => {
            var create=Rewrite(plan.CreateSql); var insert=Rewrite(plan.InsertSql);
            var createTable=Table(plan.CreateSql,create,"CREATE TABLE ",plan.Table);
            var insertTable=Table(plan.InsertSql,insert,"INSERT INTO ",plan.Table);
            if(createTable!=insertTable) throw new InvalidDataException("Unsupported SQL rule: schema and insert table differ");
            return plan with { CreateSql=create, InsertSql=insert };
        }).ToArray();
    }
}
