using System;
/// <summary>Text.</summary>
public class Periods
{
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
    /// <summary>First paragraph.<para>Second paragraph.</para></summary>
    public void ParaMissing() { }
    /// <summary>First paragraph.<para>Second paragraph.</para></summary>
    public void ParaFirstMissing() { }
    /// <summary>Intro:<para>Paragraph.</para></summary>
    public void IntroColon() { }
    /// <summary>
    /// Intro:
    /// <para>Paragraph one.</para>
    /// <para>Paragraph two.</para>
    /// </summary>
    public void Paragraphs() { }
    /// <summary>
    /// <para>Paragraph one:</para>
    /// <para>Paragraph two.</para>
    /// </summary>
    public void ParagraphColon() { }
    /// <summary>
    /// <para>Paragraph one:</para>
    /// <para>Paragraph two.</para>
    /// </summary>
    public void ParagraphColonFine() { }
    /// <summary>Empty para.<para/>Text.</summary>
    public void EmptyPara() { }
    /// <summary>
    /// <note>A note.</note>
    /// </summary>
    public void Note() { }
    /// <summary>
    /// Text.
    /// <list type="bullet"><item><description>Item</description></item></list>
    /// </summary>
    public void List() { }
    /// <summary>Entity &lt;.</summary>
    public void Entity() { }
    /// <summary>Entity &amp;.</summary>
    public void EntityAmp() { }
    /// <summary>Entity &lt;.</summary>
    public void EntityPeriod() { }
    /// <summary></summary>
    public void Empty() { }
}
