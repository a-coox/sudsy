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
//                                       ^ doc-text-ends-with-period
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
//                           ^ doc-text-ends-with-period
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
//                            ^ doc-text-ends-with-period
        public void NoPeriod() { }

        /// <summary>Has period.</summary>
        public void HasPeriod() { }

        /// <summary>Question?</summary>
//                            ^ doc-text-ends-with-period
        public void Question() { }

        /// <summary>Exclamation!</summary>
//                               ^ doc-text-ends-with-period
        public void Exclamation() { }

        /// <summary>Paren (like this.)</summary>
        public void ParenInside() { }

        /// <summary>Paren (like this).</summary>
        public void ParenOutside() { }

        /// <summary>Paren (like this)</summary>
//                                    ^ doc-text-ends-with-period
        public void ParenNone() { }

        /// <summary>Quote "like this."</summary>
//                                     ^ doc-text-ends-with-period
        public void Quote() { }

        /// <summary>Quote "like this".</summary>
        public void QuoteAfter() { }

        /// <summary>Trailing comma,</summary>
//                                 ^ doc-text-ends-with-period
        public void Comma() { }

        /// <summary>Trailing semicolon;</summary>
//                                     ^ doc-text-ends-with-period
        public void Semicolon() { }

        /// <summary>Paren (comma,)</summary>
//                               ^ doc-text-ends-with-period
        public void ParenComma() { }

        /// <summary>Paren (semicolon;)</summary>
//                                   ^ doc-text-ends-with-period
        public void ParenSemicolon() { }

        /// <summary>Trailing colon:</summary>
//                                  ^ doc-text-ends-with-period
        public void Colon() { }

        /// <summary>Text -or-</summary>
        public void Or() { }

        /// <summary>See <see cref="Periods"/></summary>
//                                            ^ doc-text-ends-with-period
        public void EndsWithSee() { }

        /// <summary>See <see cref="Periods"/>.</summary>
        public void SeeThenPeriod() { }

        /// <summary>See <see langword="null"/></summary>
//                                             ^ doc-text-ends-with-period
        public void EndsWithLangword() { }

        /// <summary>Code <c>x</c></summary>
//                                ^ doc-text-ends-with-period
        public void EndsWithCode() { }

        /// <summary>Code <c>x</c>.</summary>
        public void CodePeriod() { }

        /// <summary>Param <paramref name="a"/></summary>
//                                             ^ doc-text-ends-with-period
        public void EndsWithParamref(int a) { }

        /// <summary>Type <typeparamref name="T"/></summary>
//                                                ^ doc-text-ends-with-period
        public void EndsWithTypeparamref<T>() { }

        /// <summary><see cref="Periods"/></summary>
//                                        ^ doc-text-ends-with-period
        public void OnlySee() { }

        /// <summary>
        /// Several lines
        /// without a period
//                          ^ doc-text-ends-with-period
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
//                                                         ^ doc-text-ends-with-period
        public void ParaMissing() { }

        /// <summary>First paragraph<para>Second paragraph.</para></summary>
//                                  ^ doc-text-ends-with-period
        public void ParaFirstMissing() { }

        /// <summary>Intro:<para>Paragraph.</para></summary>
        public void IntroColon() { }

        /// <summary>
        /// Intro:
        /// <para>Paragraph one</para>
//                             ^ doc-text-ends-with-period
        /// <para>Paragraph two.</para>
        /// </summary>
        public void Paragraphs() { }

        /// <summary>
        /// <para>Paragraph one:</para>
        /// <para>Paragraph two</para>
//                             ^ doc-text-ends-with-period
        /// </summary>
        public void ParagraphColon() { }

        /// <summary>
        /// <para>Paragraph one:</para>
        /// <para>Paragraph two.</para>
        /// </summary>
        public void ParagraphColonFine() { }

        /// <summary>Empty para.<para/>Text</summary>
//                                         ^ doc-text-ends-with-period
        public void EmptyPara() { }

        /// <summary>
        /// <note>A note</note>
//                      ^ doc-text-ends-with-period
        /// </summary>
        public void Note() { }

        /// <summary>
        /// Text.
        /// <list type="bullet"><item><description>Item</description></item></list>
        /// </summary>
        public void List() { }

        /// <summary>Entity &lt;</summary>
//                             ^ doc-text-ends-with-period
        public void Entity() { }

        /// <summary>Entity &amp;</summary>
//                              ^ doc-text-ends-with-period
        public void EntityAmp() { }

        /// <summary>Entity &lt;.</summary>
        public void EntityPeriod() { }

        /// <summary></summary>
        public void Empty() { }

        /// <summary />
        public void SelfClosing() { }

        /// <summary>Text.</summary>
        /// <param name="a">Value</param>
//                               ^ doc-text-ends-with-period
        /// <returns>Result</returns>
//                         ^ doc-text-ends-with-period
        /// <remarks>Remarks</remarks>
//                          ^ doc-text-ends-with-period
        /// <exception cref="Exception">Throws</exception>
//                                            ^ doc-text-ends-with-period
        /// <seealso cref="Periods">See also</seealso>
        /// <example>Example</example>
//                          ^ doc-text-ends-with-period
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
//                                  ^ doc-text-ends-with-period
        public void Generic<T>() { }

        /// <inheritdoc />
        public override string ToString() => "";

        /// <summary>Inherited text</summary>
//                                 ^ doc-text-ends-with-period
        /// <inheritdoc />
        public override int GetHashCode() => 0;

        /// <summary>Field</summary>
//                        ^ doc-text-ends-with-period
        public int Field;

        /// <summary>Property</summary>
//                           ^ doc-text-ends-with-period
        public int Property { get; }

        /// <summary>Event</summary>
        public event EventHandler Changed;

        /// <summary>Destructor</summary>
        ~Periods() { }

        /// <summary>Enum</summary>
//                       ^ doc-text-ends-with-period
        public enum Kind
        {
            /// <summary>Member</summary>
//                             ^ doc-text-ends-with-period
            A,
        }

        /// <summary>Delegate</summary>
//                           ^ doc-text-ends-with-period
        public delegate void Callback();

        /// <summary>Operator</summary>
//                           ^ doc-text-ends-with-period
        public static Periods operator +(Periods a, Periods b) => a;

        /// <summary>Conversion</summary>
//                             ^ doc-text-ends-with-period
        public static implicit operator int(Periods a) => 0;

        /// <summary>Indexer</summary>
//                          ^ doc-text-ends-with-period
        public int this[int i] => i;

        /// <summary>Constructor</summary>
//                              ^ doc-text-ends-with-period
        public Periods() { }
    }

    /// <summary>Struct</summary>
//                     ^ doc-text-ends-with-period
    public struct Struct { }

    /// <summary>Interface</summary>
//                        ^ doc-text-ends-with-period
    public interface IFace
    {
        /// <summary>Member</summary>
//                         ^ doc-text-ends-with-period
        void Run();
    }

    /// <summary>Record</summary>
//                     ^ doc-text-ends-with-period
    public record Record(int A);
}
