// Expressive Code plugin: `squiggles="line:column:length,..."` underlines the reported spans of a
// rule example with a red wavy line; `tip="..."` is shown on hover.
import { definePlugin, ExpressiveCodeAnnotation } from 'astro-expressive-code';
import { h } from 'astro-expressive-code/hast';

class Squiggle extends ExpressiveCodeAnnotation {
  constructor(range, tip) {
    super({ inlineRange: range });
    this.tip = tip;
  }
  render({ nodesToTransform }) {
    return nodesToTransform.map((n) => h('span.squiggle', { title: this.tip }, [n]));
  }
}

const squiggles = () =>
  definePlugin({
    name: 'squiggles',
    baseStyles: '.squiggle{text-decoration:underline wavy #e5484d;text-decoration-thickness:1.5px;text-decoration-skip-ink:none;text-underline-offset:4px;cursor:help}',
    hooks: {
      annotateCode: ({ codeBlock }) => {
        const spec = codeBlock.metaOptions.getString('squiggles');
        if (!spec) return;
        const tip = codeBlock.metaOptions.getString('tip') ?? '';
        for (const s of spec.split(',')) {
          const [line, column, length] = s.split(':').map(Number);
          const l = codeBlock.getLine(line - 1);
          if (!l) continue;
          const start = Math.min(column - 1, l.text.length);
          const end = Math.min(start + length, l.text.length);
          if (end > start) l.addAnnotation(new Squiggle({ columnStart: start, columnEnd: end }, tip));
        }
      },
    },
  });

export default { plugins: [squiggles()] };
