#pragma once

#include <QtCore/QList>
#include <QtCore/QObject>
#include <QtCore/QString>

/// Where the checker took exception to something — a misspelled word or a turn of phrase,
/// which are the same thing here. What it had to say about it goes to the foot of the
/// window by way of the model, and never through here.
struct LintSpan
{
    int at = 0;
    int length = 0;

    bool operator==(const LintSpan &other) const
    {
        return at == other.at && length == other.length;
    }
};

/// What the checker objects to in one block — `markdown` for the raw source of the block
/// being edited, plain prose for one already rendered. Nothing while it is still loading.
QList<LintSpan> lintSpans(const QString &text, bool markdown);

/// Tells the highlighters when what the checker would say has changed. It takes the
/// better part of a second to load, which is longer than the window takes to appear, so
/// the blocks on screen are always drawn once without it; and a word taken into the
/// writer's dictionary is one fewer thing to mark in every block at once.
class CheckerWatch : public QObject
{
    Q_OBJECT

public:
    static CheckerWatch *instance();

    /// Take a word into the writer's own dictionary, and have the blocks looked at again.
    Q_INVOKABLE void learn(const QString &word);

Q_SIGNALS:
    void changed();

private:
    explicit CheckerWatch(QObject *parent = nullptr);

    quint64 m_generation = 0;
    bool m_ready = false;
};
