#include "markdownhighlighter.h"

#include <QtCore/QVarLengthArray>
#include <QtGui/QTextBlock>
#include <QtGui/QTextCursor>
#include <QtGui/QTextDocument>
#include <QtQml/qqml.h>

namespace {

enum Bits : quint16 {
    Bold = 1 << 0,
    Italic = 1 << 1,
    Code = 1 << 2,
    Strike = 1 << 3,
    Link = 1 << 4,
    Marker = 1 << 5,
    Hidden = 1 << 6,
};

void mark(QList<quint16> &mask, int from, int to, quint16 bits)
{
    for (int i = qMax(from, 0); i < qMin(to, int(mask.size())); ++i) {
        mask[i] |= bits;
    }
}

/// One star is italic, two is bold, three is both; a pair of tildes is a strikeout.
quint16 emphasisBits(QChar ch, int run)
{
    if (ch == '~') {
        return Strike;
    }
    if (run >= 3) {
        return Bold | Italic;
    }
    return run == 1 ? Italic : Bold;
}

int runLength(const QString &text, int at, QChar ch, int to)
{
    int run = 0;
    while (at + run < to && text.at(at + run) == ch) {
        ++run;
    }
    return run;
}

/// The start of the run of `len` `ch`s that closes a span opened before `from`, or -1.
int closingRun(const QString &text, int from, QChar ch, int len, int to)
{
    for (int i = from; i < to; ++i) {
        if (text.at(i) != ch || runLength(text, i, ch, to) < len) {
            continue;
        }
        // `a ** b` is a pair of stars, not an empty bold run.
        if (i == from || text.at(i - 1).isSpace()) {
            continue;
        }
        return i;
    }
    return -1;
}

/// The end of a `[text](url)` opening at `open`, with the label ending at `label`, or -1.
int closingLink(const QString &text, int open, int to, int &label)
{
    int depth = 0;
    int i = open;
    for (; i < to; ++i) {
        if (text.at(i) == '[') {
            ++depth;
        } else if (text.at(i) == ']' && --depth == 0) {
            break;
        }
    }
    if (i + 1 >= to || text.at(i + 1) != '(') {
        return -1;
    }
    label = i;

    depth = 0;
    for (int j = i + 1; j < to; ++j) {
        if (text.at(j) == '(') {
            ++depth;
        } else if (text.at(j) == ')' && --depth == 0) {
            return j + 1;
        }
    }
    return -1;
}

/// Walk the inline markup in [from, to), recording what each character is part of.
void scan(const QString &text, int from, int to, quint16 inherited, QList<quint16> &mask, int cursor)
{
    int i = from;
    while (i < to) {
        const QChar ch = text.at(i);
        int end = -1;
        int open = 0;
        int close = 0;
        quint16 bits = 0;
        bool nested = true;

        if (ch == '`') {
            const int run = runLength(text, i, ch, to);
            const int at = closingRun(text, i + run, ch, run, to);
            if (at > 0) {
                open = close = run;
                end = at + run;
                bits = Code;
                nested = false;
            }
        } else if (ch == '[' || (ch == '!' && i + 1 < to && text.at(i + 1) == '[')) {
            const int bracket = ch == '!' ? i + 1 : i;
            int label = -1;
            const int at = closingLink(text, bracket, to, label);
            if (at > 0 && label > bracket + 1) {
                open = bracket - i + 1;
                close = at - label;
                end = at;
                bits = Link;
            }
        } else if (ch == '*' || ch == '_' || ch == '~') {
            int run = qMin(runLength(text, i, ch, to), 3);
            if (ch == '~') {
                run = run >= 2 ? 2 : 0;
            }
            // Underscores inside a word are part of the word: `snake_case_name`.
            const bool boundary = ch != '_' || i == 0 || !text.at(i - 1).isLetterOrNumber();
            if (run > 0 && boundary && i + run < to && !text.at(i + run).isSpace()) {
                const int at = closingRun(text, i + run, ch, run, to);
                if (at > 0) {
                    open = close = run;
                    end = at + run;
                    bits = emphasisBits(ch, run);
                }
            }
        }

        if (end < 0) {
            mask[i] |= inherited;
            ++i;
            continue;
        }

        mark(mask, i, end, inherited | bits);
        // Markers stay legible while the cursor is inside the span they belong to.
        const quint16 marker = cursor >= i && cursor <= end ? Marker : Hidden;
        mark(mask, i, i + open, marker);
        mark(mask, end - close, end, marker);
        if (nested) {
            scan(text, i + open, end - close, inherited | bits, mask, cursor);
        }
        i = end;
    }
}

} // namespace

MarkdownHighlighter::MarkdownHighlighter(QObject *parent)
    : QSyntaxHighlighter(parent)
{
}

void MarkdownHighlighter::setTarget(QQuickTextDocument *target)
{
    if (m_target == target) {
        return;
    }
    m_target = target;
    QTextDocument *doc = target ? target->textDocument() : nullptr;
    if (doc) {
        // The default margin would inset the text from where its rendered self sat.
        doc->setDocumentMargin(0);
    }
    setDocument(doc);
    if (doc) {
        // Queued: the spacing is applied by editing the document, which a document must
        // never be asked to do while it is still delivering a change of its own.
        connect(doc,
                &QTextDocument::contentsChanged,
                this,
                &MarkdownHighlighter::applyLineHeight,
                Qt::QueuedConnection);
        applyLineHeight();
    }
    Q_EMIT targetChanged();
}

void MarkdownHighlighter::setCursorPosition(int position)
{
    const int previous = m_cursorPosition;
    if (previous == position) {
        return;
    }
    m_cursorPosition = position;
    restyleCursorLines(previous, position);
    Q_EMIT cursorPositionChanged();
}

void MarkdownHighlighter::setCode(bool code)
{
    if (m_code == code) {
        return;
    }
    m_code = code;
    restyle();
    Q_EMIT codeChanged();
}

void MarkdownHighlighter::setAccent(const QColor &color)
{
    if (m_accent == color) {
        return;
    }
    m_accent = color;
    restyle();
    Q_EMIT accentChanged();
}

void MarkdownHighlighter::setMuted(const QColor &color)
{
    if (m_muted == color) {
        return;
    }
    m_muted = color;
    restyle();
    Q_EMIT mutedChanged();
}

void MarkdownHighlighter::setCodeBackground(const QColor &color)
{
    if (m_codeBackground == color) {
        return;
    }
    m_codeBackground = color;
    restyle();
    Q_EMIT codeBackgroundChanged();
}

void MarkdownHighlighter::setMonoFamily(const QString &family)
{
    if (m_monoFamily == family) {
        return;
    }
    m_monoFamily = family;
    restyle();
    Q_EMIT monoFamilyChanged();
}

void MarkdownHighlighter::setCodeSize(int size)
{
    if (m_codeSize == size) {
        return;
    }
    m_codeSize = size;
    restyle();
    Q_EMIT codeSizeChanged();
}

void MarkdownHighlighter::setLineHeight(qreal height)
{
    if (qFuzzyCompare(m_lineHeight, height)) {
        return;
    }
    m_lineHeight = height;
    applyLineHeight();
    Q_EMIT lineHeightChanged();
}

/// Re-style, then take in the lines that just opened up or collapsed.
void MarkdownHighlighter::restyle()
{
    rehighlight();
    applyLineHeight();
}

/// A moved cursor only changes the lines it left and arrived at, so re-style those rather
/// than the whole block — on a long one, rehighlighting all of it costs a keystroke dearly.
/// Code is the exception: its fences open and close with the cursor anywhere inside.
void MarkdownHighlighter::restyleCursorLines(int from, int to)
{
    QTextDocument *doc = document();
    if (!doc) {
        return;
    }
    QVarLengthArray<QTextBlock, 4> lines;
    const auto add = [&lines](const QTextBlock &line) {
        if (!line.isValid()) {
            return;
        }
        for (const QTextBlock &seen : lines) {
            if (seen.blockNumber() == line.blockNumber()) {
                return;
            }
        }
        lines.append(line);
    };
    add(doc->findBlock(from));
    add(doc->findBlock(to));
    if (m_code) {
        add(doc->firstBlock());
        add(doc->lastBlock());
    }

    for (const QTextBlock &line : lines) {
        rehighlightBlock(line);
    }
    applyLineHeight();
}

void MarkdownHighlighter::highlightBlock(const QString &text)
{
    QList<quint16> mask(text.size(), 0);
    if (m_code) {
        markCode(text, mask);
    } else {
        const int base = currentBlock().position();
        markProse(text, mask, m_cursorPosition < 0 ? -1 : m_cursorPosition - base);
    }

    // A line that is nothing but hidden markers — a code fence — takes up no height. That
    // is a block format, which only the spacing pass may set.
    bool collapsed = !mask.isEmpty();
    for (const quint16 bits : mask) {
        collapsed = collapsed && (bits & Hidden);
    }
    setCurrentBlockState(collapsed ? 1 : 0);

    for (int start = 0; start < mask.size();) {
        int end = start + 1;
        while (end < mask.size() && mask[end] == mask[start]) {
            ++end;
        }
        if (mask[start] != 0) {
            setFormat(start, end - start, formatFor(mask[start]));
        }
        start = end;
    }
}

/// Inside a fenced code block only the fences themselves are markup.
void MarkdownHighlighter::markCode(const QString &text, QList<quint16> &mask) const
{
    const int last = document()->blockCount() - 1;
    const int line = currentBlock().blockNumber();
    const QString trimmed = text.trimmed();
    if ((line != 0 && line != last) || !(trimmed.startsWith("```") || trimmed.startsWith("~~~"))) {
        return;
    }
    // The block opens up again as soon as the cursor reaches either end of it.
    const int at = m_cursorPosition < 0 ? -1 : document()->findBlock(m_cursorPosition).blockNumber();
    const bool open = at == 0 || at == last;
    mark(mask, 0, text.size(), open ? Marker : Hidden);
}

void MarkdownHighlighter::markProse(const QString &text, QList<quint16> &mask, int cursor) const
{
    // A list marker is not inline markup; it stays put, and keeps its `*` out of the scan.
    int start = 0;
    while (start < text.size() && text.at(start).isSpace()) {
        ++start;
    }
    int after = start;
    if (after < text.size() && QStringLiteral("-*+").contains(text.at(after))) {
        ++after;
    } else {
        while (after < text.size() && text.at(after).isDigit()) {
            ++after;
        }
        const bool ordered = after > start && after < text.size()
            && (text.at(after) == '.' || text.at(after) == ')');
        after = ordered ? after + 1 : start;
    }
    if (after > start && after < text.size() && text.at(after) == ' ') {
        mark(mask, start, after, Marker);
        ++after;
    } else {
        after = 0;
    }

    scan(text, after, text.size(), 0, mask, cursor);
}

QTextCharFormat MarkdownHighlighter::formatFor(quint16 bits) const
{
    QTextCharFormat format;
    if (bits & Hidden) {
        // A QTextDocument cannot hide characters. Shrinking or stretching the font would,
        // but a one-pixel font poisons the glyph atlas Qt Quick shares between every item
        // on screen — text elsewhere in the window comes out as slivers. Squeezing the
        // advance instead leaves the glyphs at their own size, drawn in nothing, taking a
        // hundredth of the width they would.
        format.setFontLetterSpacingType(QFont::PercentageSpacing);
        format.setFontLetterSpacing(1);
        format.setForeground(Qt::transparent);
        return format;
    }
    if (bits & Bold) {
        format.setFontWeight(QFont::Bold);
    }
    if (bits & Italic) {
        format.setFontItalic(true);
    }
    if (bits & Strike) {
        format.setFontStrikeOut(true);
    }
    if (bits & Code) {
        format.setFontFamilies({m_monoFamily});
        format.setProperty(QTextFormat::FontPixelSize, m_codeSize);
        format.setBackground(m_codeBackground);
    }
    if (bits & Link) {
        format.setForeground(m_accent);
        format.setFontUnderline(true);
    }
    // Last, so that a revealed marker is muted rather than painted like its span.
    if (bits & Marker) {
        format.setForeground(m_muted);
    }
    return format;
}

/// TextArea has no line spacing of its own, so it is set on the document's blocks.
void MarkdownHighlighter::applyLineHeight()
{
    QTextDocument *doc = document();
    if (m_spacing || !doc || m_lineHeight <= 0) {
        return;
    }
    m_spacing = true;
    for (QTextBlock block = doc->begin(); block.isValid(); block = block.next()) {
        const bool collapsed = block.userState() == 1;
        const int type = collapsed ? QTextBlockFormat::FixedHeight : QTextBlockFormat::ProportionalHeight;
        const qreal height = collapsed ? 1 : m_lineHeight * 100;

        const QTextBlockFormat current = block.blockFormat();
        if (current.lineHeightType() == type && qFuzzyCompare(current.lineHeight(), height)) {
            continue;
        }
        QTextBlockFormat format = current;
        format.setLineHeight(height, type);
        QTextCursor cursor(block);
        // Joined so that undo takes the spacing back with the edit that provoked it.
        cursor.joinPreviousEditBlock();
        cursor.setBlockFormat(format);
        cursor.endEditBlock();
    }
    m_spacing = false;
}

void blogawrite_register_types()
{
    // Its own URI: registering into `com.blogawrite` would shadow the generated QML module.
    qmlRegisterType<MarkdownHighlighter>("com.blogawrite.text", 1, 0, "MarkdownHighlighter");
}
