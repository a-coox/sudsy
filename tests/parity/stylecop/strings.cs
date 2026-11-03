using System;

namespace Parity
{
    public class Strings
    {
        public const string Constant = "";
        private string field = "";
        private string other = string.Empty;
        private string verbatim = @"";
        private static readonly string Shared = "";
        public string Prop { get; set; } = "";
        public string Arrow => "";

        [Obsolete(message: "", error: false)]
        public string Named;

        public Strings(string p = "") { }

        [Obsolete("")]
        public void Attr() { }

        public void Parameter(string p = "") { }

        public void Use(string s)
        {
            string a = "";
            string b = s ?? "";
            if (s == "") { }
            if (s != @"") { }
            switch (s)
            {
                case "":
                    break;
            }
            if (s is "") { }
            var c = new[] { "", "x" };
            string d = $"";
            const string e = "";
            Use("");
            string f = true ? "" : s;
            var g = s switch { "" => 1, _ => 2 };
            var h = s is not "";
            var i = new { Name = "" };
            string j = "" + s;
            string k = s + "";
            Func<string, string> l = (string q) => "";
            Func<string> m = () => { return ""; };
            local();
            string local(string p = "") => "";
            const string n = "" + "";
            var o = $"{""}";
            var t = ("", 1);
            string u = s ?? @"";
            string w = default;
            Use(s: "");
            var arr = new string[] { "" };
            var pr = s is "" or "x";
            switch (s)
            {
                case "" when s.Length == 0:
                    break;
            }
        }
    }
}
