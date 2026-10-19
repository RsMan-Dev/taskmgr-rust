#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QSocketNotifier>

#include <csignal>
#include <sys/socket.h>
#include <unistd.h>

static int s_signalFds[2];

// Only write() is async-signal-safe: the event loop does the actual quit.
static void onSignal(int)
{
    char c = 1;
    (void)!write(s_signalFds[0], &c, 1);
}

static void quitOnSignals(QCoreApplication &app)
{
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, s_signalFds) != 0) {
        return;
    }
    auto *notifier = new QSocketNotifier(s_signalFds[1], QSocketNotifier::Read, &app);
    QObject::connect(notifier, &QSocketNotifier::activated, &app, [] {
        char c;
        (void)!read(s_signalFds[1], &c, 1);
        QCoreApplication::quit();
    });
    for (int sig : {SIGTERM, SIGINT, SIGHUP}) {
        std::signal(sig, onSignal);
    }
}

extern "C" int taskmgr_run(int argc, char **argv)
{
    static int s_argc = argc;
    QApplication app(s_argc, argv);
    app.setApplicationName(QStringLiteral("taskmgr"));
    app.setApplicationDisplayName(QStringLiteral("Gestionnaire des tâches"));
    app.setDesktopFileName(QStringLiteral("org.taskmgr"));
    app.setWindowIcon(QIcon::fromTheme(QStringLiteral("utilities-system-monitor")));
    QIcon::setFallbackThemeName(QStringLiteral("breeze"));
    quitOnSignals(app);

    // org.kde.desktop draws with QStyle, hence QApplication.
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
    }

    QQmlApplicationEngine engine;
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app,
                     [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine.loadFromModule("org.taskmgr", "Main");
    return app.exec();
}
