# Parse a JSON file, and create output files called source.txt and target.txt, each
# containing the values source_text and target_text from the JSON on each input line.

import sys, json

# Read the entire data set, one JSON entry per line
data = open(sys.argv[1]).readlines()
print(len(data), 'rows')

# Go through and create array of just source and target texts
outputs = []
for r in data:
    r = json.loads(r)
    o = { 'source': r['source_text'], 'target': r['target_text'] }
    outputs.append(o)

# Write to file, one entry per line
f = open('texts.json', 'w')
for r in outputs:
    f.write(json.dumps(r))
    f.write('\n')
f.close()

