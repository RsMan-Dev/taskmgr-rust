#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>
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
    app.setOrganizationName(QStringLiteral("rsman"));
    app.setOrganizationDomain(QStringLiteral("rsman.fr"));
    app.setApplicationDisplayName(QStringLiteral("Gestionnaire des tâches"));
    app.setDesktopFileName(QStringLiteral("fr.rsman.taskmgr-rust"));
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
    // Development aid: TASKMGR_PAGE / TASKMGR_SELECT pick the view,
    // TASKMGR_GRAB saves the window to a file after a few seconds and quits.
    QVariantMap initial;
    if (!qEnvironmentVariableIsEmpty("TASKMGR_PAGE")) {
        initial.insert(QStringLiteral("currentPage"), qEnvironmentVariableIntValue("TASKMGR_PAGE"));
    }
    if (!qEnvironmentVariableIsEmpty("TASKMGR_SELECT")) {
        initial.insert(QStringLiteral("initialSelection"), qEnvironmentVariable("TASKMGR_SELECT"));
    }
    engine.setInitialProperties(initial);
    engine.loadFromModule("org.taskmgr", "Main");

    const QString grab = qEnvironmentVariable("TASKMGR_GRAB");
    if (!grab.isEmpty()) {
        QTimer::singleShot(qEnvironmentVariableIntValue("TASKMGR_GRAB_DELAY") * 1000 + 8000, &app, [&engine, grab] {
            if (auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().value(0))) {
                window->grabWindow().save(grab);
            }
            QCoreApplication::quit();
        });
        if (auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().value(0));
            window && !qEnvironmentVariableIsEmpty("TASKMGR_GRAB_HEIGHT")) {
            const int w = qEnvironmentVariableIntValue("TASKMGR_GRAB_WIDTH");
            window->resize(w > 0 ? w : window->width(), qEnvironmentVariableIntValue("TASKMGR_GRAB_HEIGHT"));
        }
    }
    return app.exec();
}
