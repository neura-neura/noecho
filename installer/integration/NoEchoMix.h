#pragma once
#include <juce_core/juce_core.h>

inline bool noEchoMatches (const juce::StringArray& apps, const juce::String& path)
{
    return apps.contains (juce::File (path).getFileName(), true);
}
// Runs after voice processing, gains, ducking and final mixing. The private
// contribution uses the same captured frames and gain as the system mix.
inline float removeNoEchoContribution (float mixed, float privateSample, float gain)
{
    return mixed - privateSample * gain;
}
