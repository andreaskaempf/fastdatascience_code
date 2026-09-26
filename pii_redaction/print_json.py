import json

# Read the entire data set, one JSON entry per line
data = open('german_openpii_30k.jsonl').readlines()
print(len(data), 'rows')

# Print one sample
d = data[5]
d = json.loads(d)
for k in d.keys():
    print('\n---', k, '=', d[k])

