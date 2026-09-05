#include "markdownhighlighter.h"

#include "renderedhighlighter.h"

#include "blogawrite/src/style.cxx.h"

#include <QtGui/QTextBlock>
#include <QtGui/QTextDocument>
#include <QtQml/qqml.h>

namespace {

using blogawrite::StyleBit;

constexpr quint16 bit(StyleBit style)
{
    return quint16(style);
}

/// What each character of `text` is part of, worked out by the same markdown parser that
/// splits the document into blocks. One entry per UTF-16 unit, which is how a QString
/// counts, so it lines up with document positions as they are.
QList<quint16> styleMask(const QString &text, int cursor, bool code)
{
    const QByteArray utf8 = text.toUtf8();
    const rust::Vec<quint16> found = blogawrite::style_mask(
        rust::Str(utf8.constData(), size_t(utf8.size())), cursor, code);

    QList<quint16> mask;
    mask.reserve(qsizetype(found.size()));
    for (const quint16 bits : found) {
        mask.append(bits);
    }
    return mask;
}

} // namespace

MarkdownHighlighter::MarkdownHighlighter(QObject *parent)
    : BlockHighlighter(parent)
{
}

void MarkdownHighlighter::contentChanged()
{
    m_maskDirty = true;
    BlockHighlighter::contentChanged();
}

void MarkdownHighlighter::setCursorPosition(int position)
{
    if (m_cursorPosition == position) {
        return;
    }
    m_cursorPosition = position;
    restyleChangedLines();
    Q_EMIT cursorPositionChanged();
}

void MarkdownHighlighter::setSettled(bool settled)
{
    if (m_settled == settled) {
        return;
    }
    m_settled = settled;
    restyle();
    Q_EMIT settledChanged();
}

void MarkdownHighlighter::setCode(bool code)
{
    if (m_code == code) {
        return;
    }
    m_code = code;
    m_maskDirty = true;
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

const QList<quint16> &MarkdownHighlighter::mask()
{
    if (m_maskDirty) {
        QTextDocument *doc = document();
        m_mask = doc ? styleMask(doc->toPlainText(), m_cursorPosition, m_code) : QList<quint16>();
        m_maskDirty = false;
    }
    return m_mask;
}

/// A moved cursor opens and shuts the markers around it and leaves the rest of the block
/// exactly as it was, so re-style only the lines whose marks actually changed —
/// rehighlighting a long block on every keystroke costs it dearly. A span that runs over
/// a line break is two lines, and this finds both.
void MarkdownHighlighter::restyleChangedLines()
{
    QTextDocument *doc = document();
    if (!doc) {
        return;
    }
    const QList<quint16> before = mask();
    m_maskDirty = true;
    const QList<quint16> after = mask();

    for (QTextBlock line = doc->begin(); line.isValid(); line = line.next()) {
        // A QTextBlock's length counts the separator that ends it; its text does not.
        const int length = line.length() - 1;
        if (before.mid(line.position(), length) != after.mid(line.position(), length)) {
            rehighlightBlock(line);
        }
    }
    applyLineHeight();
}

void MarkdownHighlighter::highlightBlock(const QString &text)
{
    const QList<quint16> &whole = mask();
    const int base = currentBlock().position();
    QList<quint16> line(text.size(), 0);
    for (int i = 0; i < text.size() && base + i < whole.size(); ++i) {
        line[i] = whole[base + i];
    }

    // Only once the typing has stopped: marking a word the moment it is half-written
    // would put a wash under nearly every word on its way in. A fenced block is code
    // from end to end, and there is nothing in it for the checker to have a view on.
    if (!m_code && m_settled) {
        markLints(line);
    }

    // A line that is nothing but hidden markers — a code fence — takes up no height. That
    // is a block format, which only the spacing pass may set.
    bool collapsed = !line.isEmpty();
    for (const quint16 bits : line) {
        collapsed = collapsed && (bits & bit(StyleBit::Hidden));
    }
    setCurrentBlockState(collapsed ? Collapsed : 0);

    applyMask(line);
}

QTextCharFormat MarkdownHighlighter::formatFor(quint16 bits) const
{
    QTextCharFormat format;
    if (bits & bit(StyleBit::Hidden)) {
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
    if (bits & bit(StyleBit::Bold)) {
        format.setFontWeight(QFont::Bold);
    }
    if (bits & bit(StyleBit::Italic)) {
        format.setFontItalic(true);
    }
    if (bits & bit(StyleBit::Strike)) {
        format.setFontStrikeOut(true);
    }
    if (bits & bit(StyleBit::Code)) {
        format.merge(codeFormat());
        format.setBackground(m_codeBackground);
    }
    if (bits & bit(StyleBit::Link)) {
        format.setForeground(m_accent);
        format.setFontUnderline(true);
    }
    // Last, so that a revealed marker is muted rather than painted like its span.
    if (bits & bit(StyleBit::Marker)) {
        format.setForeground(m_muted);
    }
    return format;
}

void blogawrite_register_types()
{
    // Its own URI: registering into `com.blogawrite` would shadow the generated QML module.
    qmlRegisterType<MarkdownHighlighter>("com.blogawrite.text", 1, 0, "MarkdownHighlighter");
    qmlRegisterType<RenderedHighlighter>("com.blogawrite.text", 1, 0, "RenderedHighlighter");
    // A singleton rather than a type: there is one checker, and one moment at which it
    // becomes ready, and the foot of the window wants to hear about it.
    qmlRegisterSingletonInstance("com.blogawrite.text", 1, 0, "CheckerWatch",
                                 CheckerWatch::instance());
}
