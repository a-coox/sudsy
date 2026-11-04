using System;

namespace DocText
{
    /// <summary>Text.</summary>
    public class Duplicates
    {
        /// <summary>The value.</summary>
        /// <param name="a">The value.</param>
        /// <param name="b">The value.</param>
        /// <param name="c">The parameter is not used.</param>
        /// <param name="d">The parameter is not used.</param>
        /// <param name="e" />
        /// <param name="f" />
        /// <returns>Other.</returns>
        public int Dup(int a, int b, int c, int d, int e, int f) => 0;

        /// <summary>The  value.</summary>
        /// <remarks>The
        /// value.</remarks>
        public void Spacing() { }

        /// <summary>Inline <see cref="Duplicates"/> text.</summary>
        /// <remarks>Inline <see cref="Duplicates"/> text.</remarks>
        /// <seealso cref="Duplicates"/>
        /// <seealso cref="Duplicates"/>
        public void Inline() { }

        /// <summary>Same.</summary>
        /// <summary>Same.</summary>
        public void TwoSummaries() { }

        /// <summary>Same.</summary>
        /// <inheritdoc />
        /// <inheritdoc />
        public void Inherited() { }
    }

    /// <summary>Text.</summary>
    public class PlainComments
    {
        /// plain text on a field
        public int Field;

        /// first line
        /// second line
        public void Attached() { }

        /// <summary>Good.</summary>
        /// extra text after the summary
        public void Mixed() { }

        void Body()
        {
            /// plain in a body
            int j = 0;

            ///
            int k = 0;

            /// <summary>Not plain.</summary>
            int l = 0;
        }

        /** plain block */
        public int Block;

        //// four slashes
        public int Four;

        int last; ///trailing
    }

    /// <summary>Text.</summary>
    public class Empties
    {
        /// <summary>Text.</summary>
        /// <remarks></remarks>
        /// <example />
        /// <exception cref="System.Exception"></exception>
        /// <exception cref="System.ArgumentException">Thrown.</exception>
        /// <permission cref="System.Security.PermissionSet" />
        /// <remarks>
        /// </remarks>
        /// <remarks>Text.</remarks>
        /// <returns></returns>
        public int Elements() => 0;

        /// <summary>
        /// Text.
        /// <remarks></remarks>
        /// </summary>
        public void Nested() { }

        /// <remarks></remarks>
        int Hidden;

        /// <remarks><see cref="Hidden"/></remarks>
        public int Cref;

        /// <remarks><!-- comment --></remarks>
        public int Comment;
    }

    /// <summary>Text.</summary>
    public class Placeholders
    {
        /// <summary>
        /// <placeholder>todo</placeholder>
        /// <placeholder />
        /// </summary>
        public void Placeholder() { }

        /// <placeholder>todo</placeholder>
        private int Hidden;

        /// <summary>
        /// <para><placeholder>nested</placeholder></para>
        /// </summary>
        public int Nested;
    }

    /// <summary>Text.</summary>
    public class Periods
    {
        /// <summary>No period</summary>
        public void NoPeriod() { }

        /// <summary>Has period.</summary>
        public void HasPeriod() { }

        /// <summary>Question?</summary>
        public void Question() { }

        /// <summary>Exclamation!</summary>
        public void Exclamation() { }

        /// <summary>Paren (like this.)</summary>
        public void ParenInside() { }

        /// <summary>Paren (like this).</summary>
        public void ParenOutside() { }

        /// <summary>Paren (like this)</summary>
        public void ParenNone() { }

        /// <summary>Quote "like this."</summary>
        public void Quote() { }

        /// <summary>Quote "like this".</summary>
        public void QuoteAfter() { }

        /// <summary>Trailing comma,</summary>
        public void Comma() { }

        /// <summary>Trailing semicolon;</summary>
        public void Semicolon() { }

        /// <summary>Paren (comma,)</summary>
        public void ParenComma() { }

        /// <summary>Paren (semicolon;)</summary>
        public void ParenSemicolon() { }

        /// <summary>Trailing colon:</summary>
        public void Colon() { }

        /// <summary>Text -or-</summary>
        public void Or() { }

        /// <summary>See <see cref="Periods"/></summary>
        public void EndsWithSee() { }

        /// <summary>See <see cref="Periods"/>.</summary>
        public void SeeThenPeriod() { }

        /// <summary>See <see langword="null"/></summary>
        public void EndsWithLangword() { }

        /// <summary>Code <c>x</c></summary>
        public void EndsWithCode() { }

        /// <summary>Code <c>x</c>.</summary>
        public void CodePeriod() { }

        /// <summary>Param <paramref name="a"/></summary>
        public void EndsWithParamref(int a) { }

        /// <summary>Type <typeparamref name="T"/></summary>
        public void EndsWithTypeparamref<T>() { }

        /// <summary><see cref="Periods"/></summary>
        public void OnlySee() { }

        /// <summary>
        /// Several lines
        /// without a period
        /// </summary>
        public void MultiLine() { }

        /// <summary>
        /// Several lines
        /// with a period.
        /// </summary>
        public void MultiLinePeriod() { }

        /// <summary>
        /// Trailing blank line.
        ///
        /// </summary>
        public void TrailingBlank() { }

        /// <summary>First paragraph.<para>Second paragraph</para></summary>
        public void ParaMissing() { }

        /// <summary>First paragraph<para>Second paragraph.</para></summary>
        public void ParaFirstMissing() { }

        /// <summary>Intro:<para>Paragraph.</para></summary>
        public void IntroColon() { }

        /// <summary>
        /// Intro:
        /// <para>Paragraph one</para>
        /// <para>Paragraph two.</para>
        /// </summary>
        public void Paragraphs() { }

        /// <summary>
        /// <para>Paragraph one:</para>
        /// <para>Paragraph two</para>
        /// </summary>
        public void ParagraphColon() { }

        /// <summary>
        /// <para>Paragraph one:</para>
        /// <para>Paragraph two.</para>
        /// </summary>
        public void ParagraphColonFine() { }

        /// <summary>Empty para.<para/>Text</summary>
        public void EmptyPara() { }

        /// <summary>
        /// <note>A note</note>
        /// </summary>
        public void Note() { }

        /// <summary>
        /// Text.
        /// <list type="bullet"><item><description>Item</description></item></list>
        /// </summary>
        public void List() { }

        /// <summary>Entity &lt;</summary>
        public void Entity() { }

        /// <summary>Entity &amp;</summary>
        public void EntityAmp() { }

        /// <summary>Entity &lt;.</summary>
        public void EntityPeriod() { }

        /// <summary></summary>
        public void Empty() { }

        /// <summary />
        public void SelfClosing() { }

        /// <summary>Text.</summary>
        /// <param name="a">Value</param>
        /// <returns>Result</returns>
        /// <remarks>Remarks</remarks>
        /// <exception cref="Exception">Throws</exception>
        /// <seealso cref="Periods">See also</seealso>
        /// <example>Example</example>
        public int Elements(int a) => 0;

        /// <summary>Text.</summary>
        /// <example>
        /// <code>
        /// var x = 1
        /// </code>
        /// </example>
        public void Example() { }

        /// <summary>Text.</summary>
        /// <typeparam name="T">Type</typeparam>
        public void Generic<T>() { }

        /// <inheritdoc />
        public override string ToString() => "";

        /// <summary>Inherited text</summary>
        /// <inheritdoc />
        public override int GetHashCode() => 0;

        /// <summary>Field</summary>
        public int Field;

        /// <summary>Property</summary>
        public int Property { get; }

        /// <summary>Event</summary>
        public event EventHandler Changed;

        /// <summary>Destructor</summary>
        ~Periods() { }

        /// <summary>Enum</summary>
        public enum Kind
        {
            /// <summary>Member</summary>
            A,
        }

        /// <summary>Delegate</summary>
        public delegate void Callback();

        /// <summary>Operator</summary>
        public static Periods operator +(Periods a, Periods b) => a;

        /// <summary>Conversion</summary>
        public static implicit operator int(Periods a) => 0;

        /// <summary>Indexer</summary>
        public int this[int i] => i;

        /// <summary>Constructor</summary>
        public Periods() { }
    }

    /// <summary>Struct</summary>
    public struct Struct { }

    /// <summary>Interface</summary>
    public interface IFace
    {
        /// <summary>Member</summary>
        void Run();
    }

    /// <summary>Record</summary>
    public record Record(int A);
}
