using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;

namespace Aftercast.ActCompat
{
    public sealed record SpeechRule(string Pattern, string Replacement, bool Active);
    public sealed record SpeechRuleSnapshot(ulong Revision, SpeechRule[] Rules);
}
namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private readonly object speechRuleGate = new();
        private SpeechRule[] speechRules = [];
        private ulong speechRuleRevision = 1;
        private string? speechRuleDiskHash;
        private string SpeechRulePath => Path.Combine(AppDataFolder.FullName, "SpeechCorrections.json");
        private static readonly JsonSerializerOptions SpeechJson = new() { UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow };
        private static SpeechRule[] CompileSpeechRules(SpeechRule[] rules)
        {
            ArgumentNullException.ThrowIfNull(rules);
            if (rules.Length > 64) throw new ArgumentException("At most 64 speech rules are allowed");
            return rules.Select(rule =>
            {
                ArgumentNullException.ThrowIfNull(rule);
                if (rule.Pattern is null || rule.Replacement is null || rule.Pattern.Length > 1024 || rule.Replacement.Length > 1024 ||
                    rule.Pattern.Contains('▶') || rule.Replacement.Contains('▶') || rule.Pattern.Contains('\0') || rule.Replacement.Contains('\0'))
                    throw new ArgumentException("Invalid speech rule text");
                try { _ = new Regex(rule.Pattern, RegexOptions.IgnoreCase, TimeSpan.FromMilliseconds(100)); return rule; }
                catch (RegexParseException) { throw new ArgumentException("Invalid speech correction regular expression"); }
            }).ToArray();
        }
        internal void LoadSpeechRules()
        {
            var bytes = ReadCorrectionBytes(SpeechRulePath);
            if (bytes is null) { speechRuleDiskHash = null; return; }
            if (bytes.Length > 128 * 1024) throw new InvalidDataException("Speech rules exceed 128 KiB");
            speechRuleDiskHash = CorrectionHash(bytes);
            var rules = JsonSerializer.Deserialize<SpeechRule[]>(bytes, SpeechJson) ?? throw new InvalidDataException("Missing speech rules");
            speechRules = CompileSpeechRules(rules);
        }
        private void EnsureSpeechRuleOwner()
        {
            if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true)
                throw new InvalidOperationException("Speech rules require the owner outside callbacks");
        }
        public SpeechRuleSnapshot ReadSpeechRules()
        {
            EnsureSpeechRuleOwner();
            lock (speechRuleGate) return new(speechRuleRevision, speechRules.ToArray());
        }
        public SpeechRuleSnapshot SaveSpeechRules(ulong revision, SpeechRule[] rules)
        {
            EnsureSpeechRuleOwner();
            var compiled = CompileSpeechRules(rules);
            var bytes = JsonSerializer.SerializeToUtf8Bytes(rules, SpeechJson);
            if (bytes.Length > 128 * 1024) throw new InvalidDataException("Speech rules exceed 128 KiB");
            lock (speechRuleGate)
            {
                if (revision != speechRuleRevision) throw new InvalidOperationException("Speech rules changed; refresh before saving");
                if (CorrectionHash(ReadCorrectionBytes(SpeechRulePath)) != speechRuleDiskHash)
                    throw new InvalidOperationException("Speech rules file changed externally; restart before saving");
                var next = checked(speechRuleRevision + 1);
                WriteAtomic(SpeechRulePath, stream => stream.Write(bytes), overwrite: true);
                speechRuleDiskHash = CorrectionHash(bytes); speechRules = compiled; speechRuleRevision = next;
                return new(next, compiled.ToArray());
            }
        }
        private string CorrectSpeech(string input)
        {
            SpeechRule[] rules;
            lock (speechRuleGate) rules = speechRules;
            if (!rules.Any(item => item.Active)) return input;
            static void Validate(string text)
            {
                if (text.Length > 4096 || AudioEncoding.GetByteCount(text) > 4096) throw new InvalidDataException("Speech correction input or output exceeds 4096 bytes");
            }
            Validate(input);
            var deadline = Stopwatch.StartNew();
            foreach (var item in rules)
            {
                if (!item.Active) continue;
                if (deadline.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Speech corrections exceeded two seconds");
                try { input = Regex.Replace(input, item.Pattern, item.Replacement, RegexOptions.IgnoreCase, TimeSpan.FromMilliseconds(100)); }
                catch (RegexMatchTimeoutException) { throw new TimeoutException("Speech correction expression exceeded 100 milliseconds"); }
                Validate(input);
            }
            if (deadline.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Speech corrections exceeded two seconds");
            return input;
        }
    }
}
