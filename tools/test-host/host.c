#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>

#include "lauxlib.h"
#include "lua.h"
#include "lualib.h"

static int traceback(lua_State *state) {
    luaL_traceback(state, state, luaL_tolstring(state, 1, NULL), 1);
    return 1;
}

static int protected_call(lua_State *state, int arguments, int results) {
    int base = lua_gettop(state) - arguments;
    lua_pushcfunction(state, traceback);
    lua_insert(state, base);
    int status = lua_pcall(state, arguments, results, base);
    lua_remove(state, base);
    if (status != LUA_OK) {
        fprintf(stderr, "%s\n", lua_tostring(state, -1));
        lua_pop(state, 1);
    }
    return status;
}

static double now(void) {
    struct timespec time;
    clock_gettime(CLOCK_MONOTONIC, &time);
    return (double)time.tv_sec + (double)time.tv_nsec / 1e9;
}

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: %s <host.lua> <module directory> <package directory> [timeout seconds]\n", argv[0]);
        return 2;
    }

    double timeout = argc > 4 ? atof(argv[4]) : 60.0;
    lua_State *state = luaL_newstate();
    luaL_openlibs(state);

    if (luaL_loadfile(state, argv[1]) != LUA_OK) {
        fprintf(stderr, "%s\n", lua_tostring(state, -1));
        return 2;
    }
    lua_pushstring(state, argv[2]);
    lua_pushstring(state, argv[3]);
    if (protected_call(state, 2, 1) != LUA_OK) {
        return 1;
    }

    int host = lua_gettop(state);
    double started = now();
    double last = started;

    for (;;) {
        lua_getfield(state, host, "Finished");
        int finished = lua_toboolean(state, -1);
        lua_pop(state, 1);
        if (finished) {
            break;
        }

        if (now() - started > timeout) {
            fprintf(stderr, "the test host timed out after %.0f seconds\n", timeout);
            return 1;
        }

        double current = now();
        lua_getfield(state, host, "Tick");
        lua_pushnumber(state, (current - last) * 1000.0);
        last = current;
        if (protected_call(state, 1, 0) != LUA_OK) {
            return 1;
        }
        usleep(16000);
    }

    lua_getfield(state, host, "ExitCode");
    int exit_code = (int)lua_tointeger(state, -1);
    lua_close(state);
    return exit_code;
}
