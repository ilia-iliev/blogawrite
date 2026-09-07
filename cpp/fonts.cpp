// build.rs turns -Wmaybe-uninitialized off for the whole compiler invocation, to quiet a
// file cxx generates. Nothing is wrong with this one, so ask for it back.
#pragma GCC diagnostic warning "-Wmaybe-uninitialized"

#include <QFontDatabase>
#include <QStringList>

// Filling the font database means asking fontconfig about every face on the machine —
// five hundred of them here, and twenty milliseconds. Qt leaves it until something first
// needs a font, which is the first Text item in the QML, in the middle of the load. Qt
// guards the database with a lock of its own, so another thread can be made to pay that
// cost while the main one is still building the window.
extern "C" void blogawrite_warm_fonts()
{
    QFontDatabase::families();
}
