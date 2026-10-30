using System.Reflection.Emit;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;

// Inspect the pinned ACT IL; no speech device or native ACT window is started.
internal static class SpeechPolicy
{
    internal static object Run(string path)
    {
        using var stream = File.OpenRead(path); using var pe = new PEReader(stream); var reader = pe.GetMetadataReader();
        byte[] Body(string typeName, string methodName)
        {
            var type = reader.TypeDefinitions.Single(handle => reader.GetString(reader.GetTypeDefinition(handle).Name) == typeName);
            var method = reader.GetTypeDefinition(type).GetMethods().Select(reader.GetMethodDefinition).Single(value => reader.GetString(value.Name) == methodName);
            return pe.GetMethodBody(method.RelativeVirtualAddress).GetILBytes()!;
        }
        string Name(byte[] body, int offset)
        {
            var token = MetadataTokens.EntityHandle(BitConverter.ToInt32(body, offset + 1));
            return token.Kind switch {
                HandleKind.MethodDefinition => reader.GetString(reader.GetMethodDefinition((MethodDefinitionHandle)token).Name),
                HandleKind.MemberReference => reader.GetString(reader.GetMemberReference((MemberReferenceHandle)token).Name),
                HandleKind.FieldDefinition => reader.GetString(reader.GetFieldDefinition((FieldDefinitionHandle)token).Name),
                _ => throw new InvalidDataException("Unexpected speech IL token") };
        }
        static void Check(bool value, string name) { if (!value) throw new InvalidDataException("ACT speech policy changed: " + name); }
        void Call(byte[] body, int offset, string name)
        { Check((body[offset] == OpCodes.Call.Value || body[offset] == OpCodes.Callvirt.Value) && Name(body,offset) == name, name); }
        var tts = Body("FormActMain", "TTS");
        Call(tts, 0x00f8, "GetItemChecked");
        Check(tts[0x0122] == OpCodes.Ldc_I4.Value && BitConverter.ToInt32(tts,0x0123) == 9654, "delimiter");
        Call(tts, 0x0128, "Split");
        Check(tts[0x0133] == OpCodes.Ldc_I4_2.Value && tts[0x0147] == OpCodes.Ldc_I4_1.Value, "two fields and IgnoreCase");
        Call(tts, 0x0148, "Replace");
        Check(tts[0x0168] == OpCodes.Blt_S.Value && tts[0x016b] == OpCodes.Ldfld.Value && Name(tts,0x016b) == "PlayTtsMethod", "ordered loop and delegate");
        Call(tts, 0x0171, "Invoke");
        var misc = Body("FormActMain", "PlayMiscSound");
        Check(Name(misc, 0x0006) == "rbSndMiscBeep" && Name(misc, 0x0022) == "rbSndMiscWAV" && Name(misc, 0x004a) == "rbSndMiscTTS", "misc modes");
        Call(misc, 0x0012, "get_Beep"); Call(misc, 0x003f, "PlaySound"); Call(misc, 0x0067, "TTS"); Call(misc, 0x0076, "WriteExceptionLog");
        var sound = Body("Options_Sound", "InitializeComponent");
        Check(sound[0x239c] == OpCodes.Ldc_I4_S.Value && sound[0x239d] == 100 && sound[0x24cd] == OpCodes.Ldc_I4_S.Value && sound[0x24ce] == 100, "default volumes");
        bool HasMember(byte[] body, string name)
        {
            for (var i = 0; i + 5 <= body.Length; i++)
            {
                if (body[i] != OpCodes.Ldfld.Value && body[i] != OpCodes.Call.Value && body[i] != OpCodes.Callvirt.Value) continue;
                try { if (Name(body, i) == name) return true; } catch (Exception) { }
            }
            return false;
        }
        bool StartsChecked(string field)
        {
            for (var i = 0; i + 11 <= sound.Length; i++)
            {
                if (sound[i] != OpCodes.Ldfld.Value || sound[i + 5] != OpCodes.Ldc_I4_1.Value || sound[i + 6] != OpCodes.Callvirt.Value) continue;
                try { if (Name(sound, i) == field && Name(sound, i + 6) == "set_Checked") return true; } catch (Exception) { }
            }
            return false;
        }
        Check(StartsChecked("rbSndStartNone") && StartsChecked("rbSndWarnBeep") && StartsChecked("rbSndTimerNone"), "timer designer defaults");
        var registration = Body("FormActMain", "PopulateNewConfig");
        foreach (var field in new[] { "rbSndStartNone", "rbSndWarnBeep", "rbSndTimerNone", "tbSndStart", "tbSndWarn", "tbSndTimer" })
            Check(HasMember(registration, field), "timer setting registration " + field);
        Check(HasMember(Body("FormActMain", "InitACT"), "LoadNewSettings"), "startup settings load");
        var importSettings = Body("SettingsSerializer", "ImportFromXml");
        bool HasRadioSetter()
        {
            for (var i = 0; i + 5 <= importSettings.Length; i++)
            {
                if (importSettings[i] != OpCodes.Callvirt.Value) continue;
                try
                {
                    var handle = MetadataTokens.EntityHandle(BitConverter.ToInt32(importSettings, i + 1));
                    if (handle.Kind != HandleKind.MemberReference) continue;
                    var member = reader.GetMemberReference((MemberReferenceHandle)handle);
                    if (reader.GetString(member.Name) == "set_Checked" && member.Parent.Kind == HandleKind.TypeReference
                        && reader.GetString(reader.GetTypeReference((TypeReferenceHandle)member.Parent).Name) == "RadioButton") return true;
                }
                catch (Exception) { }
            }
            return false;
        }
        Check(HasRadioSetter(), "radio restoration");
        var timer = Body("FormSpellTimers", "TimerSound");
        foreach (var field in new[] { "rbSndStartBeep", "rbSndWarnBeep", "rbSndTimerBeep" }) Check(HasMember(timer, field), "timer cue " + field);
        Check(HasMember(timer, "IsNullOrWhiteSpace"), "timer per-definition fallback");
        return new { ordered = true, enabledOnly = true, delimiter = "▶", ignoreCase = true, soundVolume = 100, ttsVolume = 100, timerDefaults = new[] { "none", "beep", "none" }, timerSettingsRestored = true, nativeUiExecuted = false };
    }
}
