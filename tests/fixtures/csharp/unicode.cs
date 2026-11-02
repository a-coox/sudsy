// Ünïcode in a comment 🙂 中文
/// <summary>Résumé: naïve café 🙂 &amp; 中文.</summary>
class Ünïcode
{
    string s = "héllo 🙂 中文"; // trailing ünï 🙂
    string raw = """
        ünï "raw" 🙂
        """;
    int 变量 = 1; /* 块注释 🙂 */
    string i = $"{变量} é {s,-5:F2} 🙂";
#if NEVER
    ünï disabled 🙂 text
#endif
}
