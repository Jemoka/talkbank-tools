/**
 * @file send2clan_version.h
 * @brief Version information for the send2clan library
 *
 * This header defines version macros for compile-time and runtime version checks.
 *
 * @author send2clan development team
 * @version 1.0.0
 * @date 2025
 */

#ifndef SEND2CLAN_VERSION_H
#define SEND2CLAN_VERSION_H

/**
 * @brief Major version number
 *
 * Incremented for incompatible API changes (breaking changes).
 */
#define SEND2CLAN_VERSION_MAJOR 1

/**
 * @brief Minor version number
 *
 * Incremented for backwards-compatible feature additions.
 */
#define SEND2CLAN_VERSION_MINOR 0

/**
 * @brief Patch version number
 *
 * Incremented for backwards-compatible bug fixes.
 */
#define SEND2CLAN_VERSION_PATCH 0

/**
 * @brief Version string in MAJOR.MINOR.PATCH format
 */
#define SEND2CLAN_VERSION_STRING "1.0.0"

/**
 * @brief Numeric version for comparisons
 *
 * Encoded as: (MAJOR * 10000) + (MINOR * 100) + PATCH
 * Example: 1.0.0 = 10000, 1.2.3 = 10203
 */
#define SEND2CLAN_VERSION ((SEND2CLAN_VERSION_MAJOR * 10000) + \
                           (SEND2CLAN_VERSION_MINOR * 100) + \
                           SEND2CLAN_VERSION_PATCH)

/**
 * @brief Check if the library version is at least a specific version
 *
 * @param major Major version number
 * @param minor Minor version number
 * @param patch Patch version number
 * @return 1 if current version >= specified version, 0 otherwise
 *
 * Example usage:
 * @code{.c}
 * #if SEND2CLAN_VERSION_AT_LEAST(1, 0, 0)
 *     // Use features from 1.0.0 or later
 * #endif
 * @endcode
 */
#define SEND2CLAN_VERSION_AT_LEAST(major, minor, patch) \
    (SEND2CLAN_VERSION >= ((major * 10000) + (minor * 100) + patch))

#endif /* SEND2CLAN_VERSION_H */
