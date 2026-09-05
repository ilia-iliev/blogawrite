#pragma once

#include "blockhighlighter.h"

// A block that is not being edited. Everything about how it reads — the headings, the
// emphasis, the tables — is Qt's own markdown rendering; this adds the two things the
// rendering cannot know about, which are the editor's own font for code and what the
// checker made of the words.
class RenderedHighlighter : public BlockHighlighter
{
    Q_OBJECT

public:
    explicit RenderedHighlighter(QObject *parent = nullptr);

protected:
    void highlightBlock(const QString &text) override;
    QTextCharFormat formatFor(quint16 bits) const override;
    // Everything here has already been through Qt's markdown reader; what is left is prose.
    bool checksMarkdown() const override { return false; }
};
