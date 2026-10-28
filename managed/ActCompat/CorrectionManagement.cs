using System.Text;
using System.Xml;

namespace Aftercast.ActCompat
{
    public sealed record RenameDefinition(string Before, string After, bool Active);
    public sealed record RedirectDefinition(string Ability, string Destination, int Type, bool Active);
    public sealed record CorrectionSnapshot(ulong Revision, RenameDefinition[] Renames, RedirectDefinition[] Redirects, bool Editable);
}
namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private ulong correctionRevision = 1;
        private string CorrectionPath => Path.Combine(AppDataFolder.FullName, "DataCorrections.xml");
        public CorrectionSnapshot ReadCorrections()
        {
            if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true)
                throw new InvalidOperationException("Corrections require the owner outside callbacks");
            lock (CombatGate)
            {
                if (renameList.Count > 1024 || redirectList.Count > 1024) throw new InvalidDataException("Correction list exceeds 1024 rules");
                return new(correctionRevision,
                    renameList.Values.Select(rule => new RenameDefinition(rule.Before, rule.After, rule.Active)).ToArray(),
                    redirectList.Values.Select(rule => new RedirectDefinition(rule.Ability, rule.DestinationCombatant, rule.Type, rule.Active)).ToArray(), correctionFileEditable);
            }
        }
        public CorrectionSnapshot SaveCorrections(ulong revision, RenameDefinition[] renames, RedirectDefinition[] redirects)
        {
            ArgumentNullException.ThrowIfNull(renames); ArgumentNullException.ThrowIfNull(redirects);
            if (renames.Length > 1024 || redirects.Length > 1024) throw new ArgumentException("Too many correction rules");
            static void Validate(string value)
            {
                if (string.IsNullOrWhiteSpace(value) || value.Length > 512 || value.Contains('\0')) throw new ArgumentException("Invalid correction text");
            }
            var nextRenames = new SortedList<string, RenameFix>(); var nextRedirects = new SortedList<string, RedirectFix>();
            foreach (var value in renames)
            {
                ArgumentNullException.ThrowIfNull(value); Validate(value.Before); Validate(value.After);
                var rule = new RenameFix(value.Before, value.After) { Active = value.Active };
                if (!nextRenames.TryAdd(rule.ToString(), rule)) throw new ArgumentException("Duplicate rename key");
            }
            foreach (var value in redirects)
            {
                ArgumentNullException.ThrowIfNull(value); Validate(value.Ability); Validate(value.Destination);
                var rule = new RedirectFix(value.Ability, value.Destination, value.Type) { Active = value.Active };
                if (!nextRedirects.TryAdd(rule.ToString(), rule)) throw new ArgumentException("Duplicate redirect key");
            }
            if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true)
                throw new InvalidOperationException("Corrections require the owner outside callbacks");
            lock (CombatGate)
            {
                if (revision != correctionRevision) throw new InvalidOperationException("Corrections changed; refresh before saving");
                if (correctionRevision == ulong.MaxValue) throw new InvalidOperationException("Correction revision exhausted");
                if (!correctionFileEditable) throw new InvalidOperationException("Corrections contain unsupported XML; refusing to overwrite it");
                if (CorrectionHash(ReadCorrectionBytes(CorrectionPath)) != correctionDiskHash)
                    throw new InvalidOperationException("Correction file changed externally; restart or reload before saving");
                using var output = new MemoryStream();
                using (var writer = XmlWriter.Create(output, new XmlWriterSettings { Encoding = new UTF8Encoding(false), Indent = true, CloseOutput = false }))
                {
                    writer.WriteStartElement("DataCorrections"); writer.WriteStartElement("RenameFix");
                    foreach (var rule in nextRenames.Values) { writer.WriteStartElement("Rename"); writer.WriteAttributeString("Before", rule.Before); writer.WriteAttributeString("After", rule.After); writer.WriteAttributeString("Active", rule.Active.ToString()); writer.WriteEndElement(); }
                    writer.WriteEndElement(); writer.WriteStartElement("AbilityRedirectFix");
                    foreach (var rule in nextRedirects.Values) { writer.WriteStartElement("Redirect"); writer.WriteAttributeString("Ability", rule.Ability); writer.WriteAttributeString("Into", rule.DestinationCombatant); writer.WriteAttributeString("Type", rule.Type.ToString(System.Globalization.CultureInfo.InvariantCulture)); writer.WriteAttributeString("Active", rule.Active.ToString()); writer.WriteEndElement(); }
                    writer.WriteEndElement(); writer.WriteEndElement();
                }
                if (output.Length > 4 * 1024 * 1024) throw new InvalidDataException("Corrections exceed 4 MiB");
                var bytes = output.ToArray(); _ = ParseCorrections(bytes);
                WriteAtomic(CorrectionPath, stream => stream.Write(bytes), overwrite: true);
                renameList.Clear(); redirectList.Clear();
                foreach (var (key, rule) in nextRenames) renameList.Add(key, rule);
                foreach (var (key, rule) in nextRedirects) redirectList.Add(key, rule);
                correctionDiskHash = CorrectionHash(bytes); correctionFileEditable = true; correctionRevision = checked(correctionRevision + 1);
                return ReadCorrections();
            }
        }
    }
}
