using System.Reflection.Emit;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;

// Read pinned original IL only: no native UI, clipboard, file exporter or external service is executed.
internal static class ReportExportPolicy
{
    internal static object Run(string path)
    {
        using var stream=File.OpenRead(path);using var pe=new PEReader(stream);var reader=pe.GetMetadataReader();
        byte[] NamedBody(string typeName,string name,params string[] parameters)
        {
            var declaring=reader.TypeDefinitions.Single(handle=>reader.GetString(reader.GetTypeDefinition(handle).Name)==typeName);
            var method=reader.GetTypeDefinition(declaring).GetMethods().Select(reader.GetMethodDefinition).Single(method=>
                reader.GetString(method.Name)==name&&method.GetParameters().Select(parameter=>reader.GetString(reader.GetParameter(parameter).Name)).SequenceEqual(parameters));
            return pe.GetMethodBody(method.RelativeVirtualAddress).GetILBytes()!;
        }
        byte[] Body(string name,params string[] parameters)=>NamedBody("FormActMain",name,parameters);
        string Name(byte[] body,int offset)
        {
            var token=MetadataTokens.EntityHandle(BitConverter.ToInt32(body,offset+1));
            return token.Kind switch {
                HandleKind.MethodDefinition=>reader.GetString(reader.GetMethodDefinition((MethodDefinitionHandle)token).Name),
                HandleKind.MemberReference=>reader.GetString(reader.GetMemberReference((MemberReferenceHandle)token).Name),
                HandleKind.FieldDefinition=>reader.GetString(reader.GetFieldDefinition((FieldDefinitionHandle)token).Name),
                _=>throw new InvalidDataException("Unexpected original report IL token")};
        }
        static void Check(bool value,string field){if(!value)throw new InvalidDataException("Original report policy changed: "+field);}
        void Call(byte[] body,int offset,string name){Check(body[offset]==OpCodes.Call.Value||body[offset]==OpCodes.Callvirt.Value||body[offset]==OpCodes.Newobj.Value,"call opcode");Check(Name(body,offset)==name,name);}
        var mini=Body("UpdateMiniEnc");
        Call(mini,0x0042,"get_ActiveZone");Call(mini,0x0047,"get_ActiveEncounter");
        Check(Name(mini,0x010b)=="defaultTextFormat"&&Name(mini,0x0114)=="textExportFormats"&&Name(mini,0x0135)=="cbMiniColumnAlign","mini text format selection");
        Call(mini,0x015e,"GetTextExport");Call(mini,0x0173,"GetTextExport");Call(mini,0x019c,"ControlSetText");
        var end=Body("EndCombat","export");Call(end,0x0748,"get_Visible");Call(end,0x0750,"UpdateMiniEnc");
        var tick=Body("tmrTick_Tick","sender","e");
        Call(tick,0x0332,"get_InCombat");Check(tick[0x0337]==OpCodes.Brfalse.Value,"idle skips mini refresh");
        Check(Name(tick,0x0589)=="nudMiniUpdateInterval"&&tick[0x0598]==OpCodes.Rem.Value,"mini interval modulo");Call(tick,0x059f,"UpdateMiniEnc");
        var miniOptions=NamedBody("Options_MiniParse","InitializeComponent");
        Check(miniOptions[0x050d]==OpCodes.Ldc_I4_S.Value&&miniOptions[0x050e]==60&&miniOptions[0x0528]==OpCodes.Ldc_I4_1.Value&&miniOptions[0x0572]==OpCodes.Ldc_I4_5.Value,"mini interval bounds and default");
        var mainOptions=Body("InitializeComponent");Check(mainOptions[0x0e65]==OpCodes.Ldc_I4.Value&&BitConverter.ToInt32(mainOptions,0x0e66)==1000,"one second timer tick");Call(mainOptions,0x0e6a,"set_Interval");
        var grid=Body("lvDG_RetrieveVirtualItem","sender","e");
        Call(grid,0x02c1,"GetAllies");Call(grid,0x02d0,"Contains");Call(grid,0x02dc,"get_Name");
        Check(Name(grid,0x02e1)=="charName"&&grid[0x0363]==OpCodes.Brtrue_S.Value&&Name(grid,0x0371)=="GetCellForeColor"&&Name(grid,0x03be)=="ccDGAllyText","grid ally foreground precedence");
        Check(Name(grid,0x04ac)=="GetCellForeColor"&&Name(grid,0x0501)=="GetCellBackColor", "subsequent combatant cells use plugin colors");
        Check(grid[0x03db]==OpCodes.Brtrue_S.Value&&Name(grid,0x03e9)=="GetCellBackColor"&&Name(grid,0x0436)=="ccDGPersonalBackcolor","grid personal background precedence");
        var gridOptions=NamedBody("Options_ColorUserInterface","InitializeComponent");
        Check(Name(gridOptions,0x0cac)=="ccDGAllyText","default ally color target");Call(gridOptions,0x0cb1,"get_ForestGreen");Call(gridOptions,0x0cb6,"set_ForeColorSetting");
        Check(Name(gridOptions,0x0d58)=="ccDGPersonalBackcolor"&&BitConverter.ToInt32(gridOptions,0x0d5e)==200&&BitConverter.ToInt32(gridOptions,0x0d63)==255&&BitConverter.ToInt32(gridOptions,0x0d68)==200,"default personal background RGB");Call(gridOptions,0x0d6c,"FromArgb");
        var gridForeground=NamedBody("Options_ColorUserInterface","fccDataGrid_ForeColorSettingChanged","NewColor");
        Check(Name(gridForeground,0x0005)=="lvDG","grid base foreground");Call(gridForeground,0x000b,"set_ForeColor");
        var basic=Body("GetTextExport","Encounter","ExportFormatting");
        Check(basic[0x0003]==OpCodes.Ldnull.Value&&basic[0x0004]==OpCodes.Ldc_I4_0.Value,"unaligned default/font and max-lines fallback");Call(basic,0x0005,"GetTextExport");
        var core=Body("GetTextExport","RtfFormat","Encounter","ExportFormatting","TabulateFont","MaxLines");
        Check(core[0x000b]==OpCodes.Ldc_I4.Value&&BitConverter.ToInt32(core,0x000c)==256,"default LF line budget");
        Call(core,0x0065,".ctor");Call(core,0x006a,"Sort");Call(core,0x0071,"Reverse");
        Call(core,0x009e,"GetIsSelective");Call(core,0x00a6,"GetIgnoreEnemies");Call(core,0x00c2,"GetAllies");
        Check(core[0x02c6]==OpCodes.Ldstr.Value&&reader.GetUserString(MetadataTokens.UserStringHandle(BitConverter.ToInt32(core,0x02c7)&0xffffff))=="Unknown","Unknown actor exclusion");
        Check(core[0x06ee]==OpCodes.Ldc_I4_S.Value&&core[0x06ef]==10,"LF delimiter");Call(core,0x06f1,"Split");
        var table=Body("cmiEDExportXML_Click","sender","e");
        Check(table[0x0097]==OpCodes.Ldstr.Value&&reader.GetUserString(MetadataTokens.UserStringHandle(BitConverter.ToInt32(table,0x0098)&0xffffff))=="EncounterTable","table XML is distinct from encounter interchange XML");
        string Literal(byte[] body,int offset)
        {
            Check(body[offset]==OpCodes.Ldstr.Value,"literal opcode");
            return reader.GetUserString(MetadataTokens.UserStringHandle(BitConverter.ToInt32(body,offset+1)&0xffffff));
        }
        var roots=new[]{("cmiEDExportXML_Click","EncounterTable"),("cmiCDExportXML_Click","CombatantTable"),("cmiMDExportXML_Click","DamageTypeTable"),("cmiATExportXML_Click","AttackTypeTable")};
        foreach(var (method,root) in roots)
        {
            var body=Body(method,"sender","e");Check(Literal(body,0x0097)==root,"current table root");
            Check(body[0x0022]==OpCodes.Ldc_I4_1.Value&&body[0x0023]==OpCodes.Ble_S.Value,"zero/one selection keeps full table");
            Call(body,0x007f,"set_Indentation");Check(body[0x007e]==OpCodes.Ldc_I4_4.Value,"four-space XML indentation");
            Call(body,0x00f9,"get_ColHeaderCollection");Call(body,0x010e,"get_ColCollection");
        }
        var odbc=Body("ThreadOdbcExportSingle");
        foreach(var (offset,tableName) in new[]{(0x0360,"encounter_table"),(0x05c4,"combatant_table"),(0x0850,"damagetype_table"),(0x0af6,"attacktype_table"),(0x0d0c,"swing_table")})
            Check(Literal(odbc,offset)==$"INSERT INTO {tableName} ({{1}}) VALUES ({{0}});","original ODBC table and literal format");
        foreach(var offset in new[]{0x0308,0x056c,0x07f8,0x0a9e,0x0cb4})Call(odbc,offset,"SqlSafe");
        var create=NamedBody("Options_Odbc","CreateOdbcTable","odbcConnection","TableName","ColHeaders","ColTypes");
        Check(Literal(create,0x005e)=="CREATE TABLE {0} ({1});","original ODBC schema template"); Call(create,0x001d,"ToLower");
        var safe=Body("SqlSafe","input"); foreach(var offset in new[]{0x0013,0x002a,0x0041,0x0050})Call(safe,offset,"Replace");
        foreach(var rowType in new[]{"CombatantData","DamageTypeData","AttackType","MasterSwing"})
        {
            var headers=NamedBody(rowType,"get_ColHeaderCollection");Call(headers,0x0030,"get_SqlDataName");
            var cells=NamedBody(rowType,"get_ColCollection");
            Check(cells[0x0030]==OpCodes.Ldfld.Value&&Name(cells,0x0030)=="GetSqlData","XML SQL callback rather than display callback");
        }
        var html=Body("ExportTBodyHTML","atItems","BrowseNavCustomSort","BrowseNavJsArgs");
        Check(Literal(html,0x00ab)=="<th>{0}</th>"&&Literal(html,0x0132)=="<td>{0}</td>","HTML table structure");
        Call(html,0x0155,"GetColumnByName");Call(html,0x015a,"AppendFormat");
        var full=Body("ThreadExportHTML");Check(Literal(full,0x10b2).Contains("actHtml.js",StringComparison.Ordinal),"native report JavaScript resource");
        // Native full HTML walks selected encounter tree order and calls all four table exporters.
        Call(full,0x00ba,"get_Items");Call(full,0x00c1,"get_Item");
        foreach(var offset in new[]{0x0632,0x0832,0x0a66,0x0ce2})Call(full,offset,"ExportTBodyHTML");
        Check(Literal(full,0x0551)=="ed{0:000}"&&Literal(full,0x078f)=="ed{0:000}cd{1:000}"&&Literal(full,0x09b5)=="ed{0:000}cd{1:000}dt{2:000}"&&Literal(full,0x0c04)=="ed{0:000}cd{1:000}dt{2:000}at{3:000}","four nested HTML table identifiers");
        foreach(var parameter in new[]{"atItems","dtItems","edItems"})
        {
            var body=Body("ExportTBodyHTML",parameter,"BrowseNavCustomSort","BrowseNavJsArgs");
            Call(body,0x000a,".ctor");Call(body,0x000f,"Sort");Call(body,0x0027,"Reverse");
        }
        var categoryHtml=Body("ExportTBodyHTML","cdItems","BrowseNavJsArgs");
        Call(categoryHtml,0x0007,".ctor");Call(categoryHtml,0x0029,"GetItemChecked");Call(categoryHtml,0x00f4,"GetColumnByName");
        var actorHtml=Body("ExportTBodyHTML","edItems","BrowseNavCustomSort","BrowseNavJsArgs");Call(actorHtml,0x015a,"GetColumnByName");
        foreach(var (method,opcode) in new[]{("ExportXml1",OpCodes.Ldc_I4_1),("ExportXml2",OpCodes.Ldc_I4_2),("ExportXml3",OpCodes.Ldc_I4_3)})
        {var route=NamedBody("IO_XmlFile",method);Check(route[1]==opcode.Value,"three task XML depths");Call(route,2,"ExportXml");}
        var nested=NamedBody("IO_XmlFile","ExportXml","Depth");
        foreach(var (offset,root) in new[]{(0x01d5,"EncounterTable"),(0x028b,"CombatantTable"),(0x034c,"DamageTypeTable"),(0x0435,"AttackTypeTable")})Check(Literal(nested,offset)==root,"nested XML roots");
        Check(nested[0x027d]==OpCodes.Ldarg_1.Value&&nested[0x027e]==OpCodes.Ldc_I4_1.Value&&nested[0x027f]==OpCodes.Ble.Value,"depth one stops after main table");
        Check(nested[0x0427]==OpCodes.Ldarg_1.Value&&nested[0x0428]==OpCodes.Ldc_I4_2.Value&&nested[0x0429]==OpCodes.Ble.Value,"depth two stops before action rows");
        Check(nested[0x04fb]==OpCodes.Brtrue_S.Value,"first action SQL index override");Call(nested,0x0510,"get_EncId");
        var id=NamedBody("EncounterData","get_EncId");Call(id,0x0011,"GetHashCode");Check(Literal(id,0x0019)=="x8","cached object hash format");
        var caption=NamedBody("EncounterData","ToString");Check(Literal(caption,0x0012)=="{0} - [{1}]"&&Literal(caption,0x004c)=="{0} - [{1}] ({3}) {2}"&&Literal(caption,0x0091)=="{0} - [{1}] {2}","encounter caption formats");
        var controls=NamedBody("IO_ExportHtml","InitializeComponent");
        foreach(var (offset,value) in new[]{(0x064c,1600),(0x066a,128),(0x06b9,256),(0x0705,1600),(0x0723,128),(0x0772,640)})
            Check(controls[offset]==OpCodes.Ldc_I4.Value&&BitConverter.ToInt32(controls,offset+1)==value,"HTML graph dimensions");
        return new {text=new{unaligned=true,descendingSort=true,defaultLines=256,unknownActorExcluded=true,selectiveIgnoreEnemies="all actors",lfFraming=true},tableXmlRoots=roots.Select(value=>value.Item2).ToArray(),xmlValues="SQL names and callbacks",xmlSelection="more than one filters",hierarchyXmlDepths=new[]{1,2,3},hierarchyActionFirstField="EncId",htmlDepths=4,graphPixels=new{minimum=128,maximum=1600,width=640,height=256},htmlNativeResources=true,nativeUiExecuted=false,clipboardAccessExecuted=false,exportFileExecuted=false};
    }
}
