#include "lint.h"

#include "blogawrite/src/lint.cxx.h"

#include <QtCore/QTimer>

QList<LintSpan> lintSpans(const QString &text, bool markdown)
{
    if (text.isEmpty() || !blogawrite::checker_ready()) {
        return {};
    }
    const QByteArray utf8 = text.toUtf8();
    const rust::Vec<blogawrite::Lint> found =
        blogawrite::request_check(rust::Str(utf8.constData(), size_t(utf8.size())), markdown);

    QList<LintSpan> spans;
    spans.reserve(qsizetype(found.size()));
    for (const blogawrite::Lint &lint : found) {
        spans.append(LintSpan{int(lint.at), int(lint.len)});
    }
    return spans;
}

void CheckerWatch::learn(const QString &word)
{
    if (word.isEmpty()) {
        return;
    }
    const QByteArray utf8 = word.toUtf8();
    blogawrite::learn(rust::Str(utf8.constData(), size_t(utf8.size())));
    Q_EMIT changed();
}

CheckerWatch *CheckerWatch::instance()
{
    static CheckerWatch *watch = new CheckerWatch;
    return watch;
}

CheckerWatch::CheckerWatch(QObject *parent)
    : QObject(parent)
{
    // Crossing from the worker into Qt would require a second callback bridge. Polling one
    // integer is cheaper and keeps every QObject on the UI thread.
    QTimer *clock = new QTimer(this);
    clock->setInterval(100);
    connect(clock, &QTimer::timeout, this, [this] {
        const bool ready = blogawrite::checker_ready();
        const quint64 generation = blogawrite::checker_generation();
        if (generation == m_generation && ready == m_ready) {
            return;
        }
        const bool becameReady = ready && !m_ready;
        m_generation = generation;
        m_ready = ready;
        if (becameReady || ready) {
            Q_EMIT changed();
        }
    });
    clock->start();
}
